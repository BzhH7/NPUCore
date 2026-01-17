use super::__switch;
use super::{fetch_task, add_task, sleep_interruptible, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use super::task::TASK_NOT_RUNNING;
use crate::hal::{TrapContext, disable_interrupts, restore_interrupts};
use crate::timer::get_time_ns;
use alloc::sync::Arc;
use lazy_static::*;
use spin::Mutex;
use core::arch::asm;
use core::sync::atomic::Ordering;
use crate::config::MAX_CPU_NUM;
use alloc::vec::Vec;

/// 处理器对象
pub struct Processor {
    /// 当前正在运行的任务
    current: Option<Arc<TaskControlBlock>>,
    /// 空闲任务的上下文，用于在任务切换时保存和恢复状态
    idle_task_cx: TaskContext,
    /// 等待被加入就绪队列的任务（上下文已保存，等待被重新调度）
    /// 用于解决多核竞争问题：任务上下文保存后才能被其他CPU偷取
    pending_task: Option<Arc<TaskControlBlock>>,
}

impl Processor {
    /// 构造函数
    pub fn new() -> Self {
        Self {
            // 初始化时处理器为空闲
            current: None,
            // 空闲任务的上下文
            idle_task_cx: TaskContext::zero_init(),
            // 等待加入队列的任务
            pending_task: None,
        }
    }
    /// 获取空闲任务的上下文指针
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }
    /// 取出当前正在运行的任务
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        // 将current字段置空，并返回其中的值
        self.current.take()
    }
    /// 获取当前正在运行的任务的克隆
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
    /// 检查当前 Processor 是否为空闲
    pub fn is_vacant(&self) -> bool {
        self.current.is_none()
    }
    /// 设置待加入就绪队列的任务
    pub fn set_pending(&mut self, task: Arc<TaskControlBlock>) {
        self.pending_task = Some(task);
    }
    /// 取出待加入就绪队列的任务
    pub fn take_pending(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.pending_task.take()
    }
}

lazy_static! {
    /// 全局的处理器对象
    /// 使用 Mutex 包装以确保多线程安全
    // pub static ref PROCESSOR: Mutex<Processor> = Mutex::new(Processor::new());
    pub static ref PROCESSORS: Vec<Mutex<Processor>> = {
        let mut v = Vec::new();
        for _ in 0..MAX_CPU_NUM {
            v.push(Mutex::new(Processor::new()));
        }
        v
    };
}

/// 运行任务调度

pub fn run_tasks() {
    loop {
        let cpu_id = current_cpu_id();
        
        // 【关键检查】验证 tp 寄存器有效
        // 如果 tp 无效，说明之前的上下文切换出了问题
        if cpu_id >= MAX_CPU_NUM {
            panic!("[run_tasks] Invalid cpu_id {} (tp register corrupted)!", cpu_id);
        }
        
        // 1. 【关键】获取锁之前必须关闭中断，防止中断处理函数重入导致死锁
        disable_interrupts();

        let mut processor = PROCESSORS[cpu_id].lock();
        
        // 【关键修复】先检查是否有pending任务需要处理
        // 这个任务的上下文已经在上次__switch时保存了
        if let Some(pending) = processor.take_pending() {
            // 【关键】清除 running_on_cpu 标记，表示任务已经停止运行
            pending.running_on_cpu.store(TASK_NOT_RUNNING, Ordering::SeqCst);
            // 【关键】清除 on_cpu 标记，表示任务已完成上下文切换
            // 这允许其他 CPU 通过 work stealing 偷取该任务
            pending.on_cpu.store(false, Ordering::Release);
            
            // 内存屏障确保上述标记对其他 CPU 可见
            core::sync::atomic::fence(Ordering::SeqCst);
            
            // CFS: 更新被切换出去任务的vruntime
            {
                let now = get_time_ns() as u64;
                let mut inner = pending.acquire_inner_lock();
                inner.sched_entity.update_runtime(now);
            }
            
            // 根据任务状态决定加入哪个队列
            let status = pending.acquire_inner_lock().task_status;
            drop(processor); // 先释放锁再操作队列，避免锁顺序问题
            
            match status {
                TaskStatus::Ready => {
                    // 正常的 suspend 调用，加入就绪队列
                    add_task(pending);
                }
                TaskStatus::Interruptible => {
                    // block 调用，加入可中断等待队列
                    sleep_interruptible(pending);
                }
                _ => {
                    // 其他状态不应该出现在 pending 中
                    panic!("[CPU {}] pending task has unexpected status: {:?}", cpu_id, status);
                }
            }
            processor = PROCESSORS[cpu_id].lock();
        }
        
        if let Some(task) = fetch_task() {
            // 【关键】参考 starry-mix: 等待任务完成上一次的调度过程
            // 如果任务的 on_cpu 仍为 true，说明上一个 CPU 还没完成切换
            // 这在 work stealing 场景下尤为重要
            let mut spin_count = 0u32;
            while task.on_cpu.load(Ordering::Acquire) {
                core::hint::spin_loop();
                spin_count += 1;
                if spin_count > 1000000 {
                    // 如果等待太久，可能有问题
                    log::warn!("[CPU {}] Waiting too long for task pid={} to finish on_cpu", 
                               cpu_id, task.pid.0);
                    spin_count = 0;
                }
            }
            
            // 【关键】原子检查：确保任务不会同时在多个CPU上运行
            let prev_cpu = task.running_on_cpu.compare_exchange(
                TASK_NOT_RUNNING,
                cpu_id,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
            if let Err(other_cpu) = prev_cpu {
                panic!("[CPU {}] DOUBLE RUN DETECTED! Task pid={} is already running on CPU {}!", 
                       cpu_id, task.pid.0, other_cpu);
            }
            
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            let next_task_cx_ptr = {
                let mut task_inner = task.acquire_inner_lock();
                
                // 【关键】确保 kernel_tp 设置为当前 CPU ID
                // 这对于 work stealing 场景尤为重要，因为偷取的任务可能来自其他 CPU
                let trap_cx = task_inner.get_trap_cx();
                let old_kernel_tp = trap_cx.kernel_tp;
                trap_cx.kernel_tp = cpu_id;
                
                // 【调试】验证 kernel_tp 设置成功
                let new_kernel_tp = trap_cx.kernel_tp;
                if new_kernel_tp != cpu_id {
                    panic!("[CPU {}] Failed to set kernel_tp: expected {} but got {}", 
                           cpu_id, cpu_id, new_kernel_tp);
                }
                if old_kernel_tp != cpu_id && old_kernel_tp < MAX_CPU_NUM {
                    log::trace!("[CPU {}] Updated kernel_tp from {} to {} for task pid={}", 
                               cpu_id, old_kernel_tp, cpu_id, task.pid.0);
                }
                
                task_inner.task_status = TaskStatus::Running;
                // CFS: 记录任务开始执行的时间
                task_inner.sched_entity.exec_start = get_time_ns() as u64;
                // Wake-up Affinity: 记录任务当前运行的CPU
                task_inner.sched_entity.set_last_cpu(cpu_id);
                &task_inner.task_cx as *const TaskContext
            };
            
            // Debug: Check next_task_cx before switch
            let next_ra = unsafe { (*next_task_cx_ptr).ra };
            let next_sp = unsafe { (*next_task_cx_ptr).sp };
            let task_pid = task.pid.0;
            
            if next_ra == 0 {
                panic!("[CPU {}] About to switch to task pid={} with ra=0x0!", 
                       cpu_id, task_pid);
            }
            if next_ra < 0x80000000 || next_ra > 0xffffffff00000000 {
                panic!("[CPU {}] About to switch to task pid={} with invalid ra=0x{:x}!", 
                       cpu_id, task_pid, next_ra);
            }
            
            // 【调试日志】记录即将切换的任务
            // 使用 print! 而不是 log::info! 以确保立即输出
            // print!("[CPU {}] SW->pid={} ra={:#x}\n", cpu_id, task_pid, next_ra);
            
            // 【关键】设置 on_cpu 标记，表示任务正在进行上下文切换
            // 这防止其他 CPU 在切换完成前偷取该任务
            task.on_cpu.store(true, Ordering::Release);
            
            // Memory barrier to ensure on_cpu is visible before __switch
            core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
            
            processor.current = Some(task);
            drop(processor);
            
            // 2. 切换任务
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
            // 回到这里时，任务已被挂起，pending_task已设置（如果是正常suspend）
            
            // 【安全检查】验证 __switch 返回后 tp 寄存器仍有效
            #[cfg(target_arch = "riscv64")]
            {
                let current_tp: usize;
                unsafe { core::arch::asm!("mv {}, tp", out(reg) current_tp); }
                if current_tp >= MAX_CPU_NUM {
                    panic!("[run_tasks] After __switch: tp={} is invalid!", current_tp);
                }
            }
            // 继续循环会处理pending_task
        } else {
            // 没有任务，释放锁
            drop(processor);

            // 【Idle 状态处理】
            // 必须开启中断才能被唤醒（响应时钟中断或其他）
            restore_interrupts(true);
            
            // 可选：使用 wfi 等待以降低功耗
            // riscv::asm::wfi();
        }
    }
}

pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    let cpu_id = current_cpu_id();
    let was_enabled = disable_interrupts();
    let task = PROCESSORS[cpu_id].lock().take_current();
    restore_interrupts(was_enabled);
    task
}

pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    let cpu_id = current_cpu_id();
    // 【关键检查】验证 cpu_id 有效
    if cpu_id >= MAX_CPU_NUM {
        panic!("[current_task] Invalid cpu_id {} (tp register corrupted)!", cpu_id);
    }
    // 1. 关中断以获取锁
    let was_enabled = disable_interrupts();
    let task = PROCESSORS[cpu_id].lock().current();
    // 3. 仅在进入前是开启状态时，才恢复中断
    restore_interrupts(was_enabled);
    // 如果之前是关闭的（如在 trap_handler 中），则保持关闭
    task
}

/// 获取当前正在运行的任务的用户态页表令牌
pub fn current_user_token() -> usize {
    // 【关键修复】防止 Idle 时 Panic
    match current_task() {
        Some(task) => task.get_user_token(),
        None => {
            // 如果是 Idle 状态被中断（如时钟中断），此时没有用户页表。
            // 返回 0 可能意味着使用内核页表（取决于你的 MMU 逻辑），或者应该在调用处避免调用此函数。
            // 为了防止 Panic，我们这里返回 0，并在日志里报个警（可选）
            0 
        }
    }
}

/// 获取当前正在运行的任务的陷阱上下文
pub fn current_trap_cx() -> &'static mut TrapContext {
    // 【关键修复】防止 Idle 时 Panic
    match current_task() {
        Some(task) => task.acquire_inner_lock().get_trap_cx(),
        None => {
            panic!("Trap Context not found! (Running Idle?)");
        }
    }
}

pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let cpu_id = current_cpu_id();
    
    // Sanity check: verify CPU id is valid
    if cpu_id >= crate::config::MAX_CPU_NUM {
        panic!("[schedule] Invalid cpu_id={}! MAX_CPU_NUM={}", cpu_id, crate::config::MAX_CPU_NUM);
    }
    
    // 【关键修复】关中断防止死锁
    disable_interrupts();
    
    let idle_task_cx_ptr = PROCESSORS[cpu_id].lock().get_idle_task_cx_ptr();
    
    // Debug: Check idle_task_cx before switching back
    let idle_ra = unsafe { (*idle_task_cx_ptr).ra };
    let idle_sp = unsafe { (*idle_task_cx_ptr).sp };
    if idle_ra == 0 {
        panic!("[CPU {}] schedule(): idle_task_cx has ra=0x0! sp=0x{:x}", cpu_id, idle_sp);
    }
    if idle_ra < 0x80000000 || idle_ra > 0xffffffff00000000 {
        panic!("[CPU {}] schedule(): idle_task_cx has invalid ra=0x{:x}!", cpu_id, idle_ra);
    }
    
    // 切换回 idle 循环
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
        // 回来后，说明任务又被调度了，恢复中断（可选，通常由 sstatus 自动恢复）
        // sstatus::set_sie(); 
    }
    // log::info!("[schedule] Back from idle (Resumed)!");
}

pub fn current_cpu_id() -> usize {
    #[cfg(target_arch = "riscv64")]
    {
        let cpu_id: usize;
        unsafe {
            asm!("mv {}, tp", out(reg) cpu_id);
        }
        cpu_id
    }
    #[cfg(target_arch = "loongarch64")]
    {
        use crate::hal::arch::loongarch64::register::CPUId;
        CPUId::read().get_core_id()
    }
}