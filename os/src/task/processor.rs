use super::{__switch, do_wake_expired};
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::hal::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;
use spin::Mutex;
use core::arch::asm;
use crate::config::MAX_CPU_NUM; // 引入CPU数量配置
use alloc::vec::Vec;

/// 处理器对象
pub struct Processor {
    /// 当前正在运行的任务
    current: Option<Arc<TaskControlBlock>>,
    /// 空闲任务的上下文，用于在任务切换时保存和恢复状态
    idle_task_cx: TaskContext,
}

impl Processor {
    /// 构造函数
    pub fn new() -> Self {
        Self {
            // 初始化时处理器为空闲
            current: None,
            // 空闲任务的上下文
            idle_task_cx: TaskContext::zero_init(),
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
// 引用 sstatus
use riscv::register::sstatus;

pub fn run_tasks() {
    loop {
        let cpu_id = current_cpu_id();
        
        // 1. 【关键】获取锁之前必须关闭中断，防止中断处理函数重入导致死锁
        unsafe { sstatus::clear_sie(); }

        let mut processor = PROCESSORS[cpu_id].lock();
        
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            let next_task_cx_ptr = {
                let mut task_inner = task.acquire_inner_lock();
                task_inner.get_trap_cx().kernel_tp = cpu_id;
                task_inner.task_status = TaskStatus::Running;
                &task_inner.task_cx as *const TaskContext
            };
            processor.current = Some(task);
            drop(processor);
            
            // 2. 切换任务
            // __switch 恢复的 sstatus 通常会包含开启中断（如果任务是在开启中断时被挂起的）
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            // 没有任务，释放锁
            drop(processor);

            // 3. 【关键】Idle 状态处理
            // 必须开启中断才能被唤醒（响应时钟中断或其他），
            // 使用 wfi 等待以降低功耗。
            unsafe {
                sstatus::set_sie(); 
                riscv::asm::wfi(); 
            }
        }
    }
}

pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    let cpu_id = current_cpu_id();
    let sstatus = unsafe { riscv::register::sstatus::read() };
    let was_enabled = sstatus.sie();
    unsafe { riscv::register::sstatus::clear_sie(); }
    let task = PROCESSORS[cpu_id].lock().take_current();
    if was_enabled {
        unsafe { riscv::register::sstatus::set_sie(); }
    }
    task
}

pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    let cpu_id = current_cpu_id();
    // 1. 获取当前 sstatus 状态
    let sstatus = unsafe { riscv::register::sstatus::read() };
    let was_enabled = sstatus.sie();
    // 2. 关中断以获取锁
    unsafe { riscv::register::sstatus::clear_sie(); }
    let task = PROCESSORS[cpu_id].lock().current();
    // 3. 仅在进入前是开启状态时，才恢复中断
    if was_enabled {
        unsafe { riscv::register::sstatus::set_sie(); }
    }
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
    
    // 【关键修复】关中断防止死锁
    unsafe { sstatus::clear_sie(); }
    
    let idle_task_cx_ptr = PROCESSORS[cpu_id].lock().get_idle_task_cx_ptr();
    
    // 切换回 idle 循环
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
        // 回来后，说明任务又被调度了，恢复中断（可选，通常由 sstatus 自动恢复）
        // sstatus::set_sie(); 
    }
}

pub fn current_cpu_id() -> usize {
    let cpu_id: usize;
    #[cfg(target_arch = "riscv64")]
    unsafe {
        asm!("mv {}, tp", out(reg) cpu_id);
    }
    #[cfg(not(target_arch = "riscv64"))]
    {
        cpu_id = 0;
    }
    cpu_id
}