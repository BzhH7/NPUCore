mod context;
mod elf;
mod manager;
pub mod pid;
pub mod processor;
pub mod signal;
pub mod state_machine;
pub mod task;
pub mod threads;

use crate::hal::__switch;
 use crate::hal::disable_interrupts;
use crate::{
    fs::{OpenFlags, ROOT_FD},
    mm::translated_refmut,
    utils::InterruptGuard,
};
use alloc::{collections::VecDeque, sync::Arc};
pub use context::TaskContext;
pub use elf::{load_elf_interp, AuxvEntry, AuxvType, ELFInfo};
use lazy_static::*;
use log::warn;
use manager::fetch_task;
pub use manager::{
    add_task, do_oom, do_wake_expired, find_task_by_pid, find_task_by_tgid, procs_count,
    sleep_interruptible, wait_with_timeout, wake_interruptible,
};
// pub use pid::RecycleAllocator;
pub use pid::{pid_alloc, trap_cx_bottom_from_tid, ustack_bottom_from_tid, PidHandle};
pub use processor::{
    current_task, current_trap_cx, current_user_token, run_tasks, schedule, take_current_task,
};
pub use signal::*;
pub use task::{RobustList, Rusage, TaskControlBlock, TaskStatus};
use self::processor::{PROCESSORS, current_cpu_id};

#[allow(unused)]
pub fn try_yield() {
    let cpu_id = current_cpu_id();
    let lock = PROCESSORS[cpu_id].lock();
    let mut do_suspend = false;
    if !lock.is_vacant() {
        do_suspend = true;
    }
    drop(lock);
    if do_suspend {
        suspend_current_and_run_next()
    }
}

pub fn suspend_current_and_run_next() {
    let _guard = InterruptGuard::new();
    let cpu_id = processor::current_cpu_id();

    if let Some(task) = take_current_task() {
        let task_cx_ptr = {
            let mut task_inner = task.acquire_inner_lock();
            let ptr = &mut task_inner.task_cx as *mut TaskContext;
            task_inner.task_status = TaskStatus::Ready;
            ptr
        };

        // 【关键修复】不直接add_task，而是设置pending_task
        // 这样任务上下文会在__switch时保存，之后才被加入就绪队列
        // 避免其他CPU在上下文保存前就偷取任务导致竞争
        {
            let mut processor = processor::PROCESSORS[cpu_id].lock();
            processor.set_pending(task);
        }
        
        schedule(task_cx_ptr);
        
        // Debug: check ra after resuming from schedule
        let ra_after: usize;
        unsafe { core::arch::asm!("mv {}, ra", out(reg) ra_after); }
        if ra_after == 0 {
            panic!("[CPU {}] suspend_current_and_run_next: ra=0 after schedule()!", cpu_id);
        }
    }
}

pub fn block_current_and_run_next() {
    let _guard = InterruptGuard::new();
    let cpu_id = processor::current_cpu_id();
    
    let task = take_current_task().unwrap();
    
    let task_cx_ptr = {
        let mut task_inner = task.acquire_inner_lock();
        let ptr = &mut task_inner.task_cx as *mut TaskContext;
        task_inner.task_status = TaskStatus::Interruptible;
        ptr
    };
    
    // 【关键修复】不直接 sleep_interruptible，而是设置 pending_task
    // 这样任务上下文会在 __switch 时保存，之后才被加入睡眠队列
    // 避免其他CPU在上下文保存前就唤醒并运行任务导致竞争
    {
        let mut processor = processor::PROCESSORS[cpu_id].lock();
        processor.set_pending(task);
    }
    
    // 在 schedule 之后, run_tasks 会检测到 pending_task
    // 但 block 不是加入 ready 队列，而是加入 interruptible 队列
    // 所以需要标记这个 task 是要 block 而不是 ready
    schedule(task_cx_ptr);
}

pub fn do_exit(task: Arc<TaskControlBlock>, exit_code: u32) {
    // 多核安全重构：避免嵌套锁导致死锁
    // 策略：分阶段执行，每阶段只持有一把锁
    
    // === 阶段1：收集需要的信息并设置基本状态 ===
    let (need_signal_parent, parent_task_opt, children_to_move, clear_child_tid, user_token) = {
        let mut inner = task.acquire_inner_lock();
        
        // 设置 zombie 状态和 exit_code
        inner.task_status = TaskStatus::Zombie;
        inner.exit_code = exit_code;
        
        // 收集父任务信息
        let parent = if !task.exit_signal.is_empty() {
            inner.parent.as_ref().and_then(|p| p.upgrade())
        } else {
            None
        };
        
        // 收集子任务列表（move out）
        let children: VecDeque<Arc<TaskControlBlock>> = inner.children.drain(..).collect();
        
        let clear_tid = inner.clear_child_tid;
        let token = task.get_user_token();
        
        (task.exit_signal, parent, children, clear_tid, token)
    };
    // inner lock released here
    
    // === 阶段2：通知父任务 ===
    if !need_signal_parent.is_empty() {
        if let Some(parent_task) = parent_task_opt {
            let need_wake = {
                let mut parent_inner = parent_task.acquire_inner_lock();
                parent_inner.add_signal(need_signal_parent);
                
                if parent_inner.task_status == TaskStatus::Interruptible {
                    parent_inner.task_status = TaskStatus::Ready;
                    true
                } else {
                    false
                }
            };
            // parent_inner lock released here
            
            if need_wake {
                wake_interruptible(parent_task);
            }
        } else {
            warn!("[do_exit] parent is None");
        }
    }
    
    // === 阶段3：将子任务移交给 initproc ===
    if !children_to_move.is_empty() {
        // 先更新每个子任务的 parent 指针
        for child in children_to_move.iter() {
            let mut child_inner = child.acquire_inner_lock();
            child_inner.parent = Some(Arc::downgrade(&INITPROC));
        }
        
        // 然后更新 initproc 的子任务列表
        let need_wake_initproc = {
            let mut initproc_inner = INITPROC.acquire_inner_lock();
            for child in children_to_move {
                initproc_inner.children.push(child);
            }
            
            if initproc_inner.task_status == TaskStatus::Interruptible {
                initproc_inner.task_status = TaskStatus::Ready;
                true
            } else {
                false
            }
        };
        // initproc_inner lock released here
        
        if need_wake_initproc {
            wake_interruptible(INITPROC.clone());
        }
    }
    
    // === 阶段4：处理 clear_child_tid (futex) ===
    if clear_child_tid != 0 {
        log::debug!(
            "[do_exit] do futex wake on clear_child_tid: {:X}",
            clear_child_tid
        );
        match translated_refmut(user_token, clear_child_tid as *mut u32) {
            Ok(phys_ref) => {
                *phys_ref = 0;
                task.futex.lock().wake(phys_ref as *const u32 as usize, 1);
            }
            Err(_) => log::warn!("invalid clear_child_tid"),
        };
    }
    
    // === 阶段5：释放用户资源 ===
    {
        let mut vm_lock = task.vm.lock();
        vm_lock.dealloc_user_res(task.tid);
        if Arc::strong_count(&task.vm) == 1 {
            vm_lock.recycle_data_pages();
        }
    }
    
    log::trace!(
        "[do_exit] Pid {} exited with {}",
        task.pid.0,
        exit_code
    );
}

pub fn exit_current_and_run_next(exit_code: u32) -> ! {
    // ==== 关键修复：关中断 ====
    disable_interrupts();

    // take from Processor
    let task = take_current_task().unwrap();
    do_exit(task, exit_code);
    // we do not have to save task context
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
    panic!("Unreachable");
}

pub fn exit_group_and_run_next(exit_code: u32) -> ! {
    // ==== 关键修复：关中断 ====
    disable_interrupts();

    let task = take_current_task().unwrap();
    let tgid = task.tgid;
    do_exit(task, exit_code);

    let mut exit_list = VecDeque::new();

    // 遍历所有 CPU 的管理器
    use manager::TASK_MANAGERS; 
    
    for manager_mutex in TASK_MANAGERS.iter() {
        let mut manager = manager_mutex.lock();
        let mut remain = manager.ready_queue.len();
        while let Some(task) = manager.ready_queue.pop_front() {
            if task.tgid == tgid {
                exit_list.push_back(task);
            } else {
                manager.ready_queue.push_back(task);
            }
            remain -= 1;
            if remain == 0 { break; }
        }
        
        let mut remain = manager.interruptible_queue.len();
        while let Some(task) = manager.interruptible_queue.pop_front() {
            if task.tgid == tgid {
                exit_list.push_back(task);
            } else {
                manager.interruptible_queue.push_back(task);
            }
            remain -= 1;
            if remain == 0 { break; }
        }
    }

    for task in exit_list.into_iter() {
        do_exit(task, exit_code);
    }
    
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
    panic!("Unreachable");
}

lazy_static! {
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new({
        let elf = ROOT_FD.open("initproc", OpenFlags::O_RDONLY, true).unwrap();
        TaskControlBlock::new(elf)
    });
}

pub fn add_initproc() {
    println!("[add_initproc] Entering function...");
    println!("[add_initproc] About to access INITPROC lazy_static...");
    let initproc_pid = INITPROC.pid.0;
    println!("[add_initproc] INITPROC pid={}", initproc_pid);
    add_task(INITPROC.clone());
    println!("[add_initproc] INITPROC added successfully");
}

/// 初始化任务子系统的全局数据结构
/// 必须在多核启动前由 BSP 调用，以避免多核竞争初始化 lazy_static 导致的死锁
pub fn init_task_subsystem() {
    use manager::{TASK_MANAGERS, TIMEOUT_WAITQUEUE};
    use processor::PROCESSORS;
    // 触发 lazy_static 初始化（只读访问即可）
    let _ = PROCESSORS.len();
    let _ = TASK_MANAGERS.len();
    let _ = TIMEOUT_WAITQUEUE.lock();
}
