pub mod context;
use core::arch::{asm, global_asm};

use super::TrapImpl;
use crate::config::TRAMPOLINE;
use crate::fs::directory_tree::ROOT;
use crate::fs::OpenFlags;
use crate::hal::arch::riscv::time::set_next_trigger;
use crate::mm::{frame_reserve, MemoryError, VirtAddr};
use crate::syscall::syscall;
use crate::task::{
    current_task, current_trap_cx, do_signal, do_wake_expired, run_tasks, suspend_current_and_run_next,
    Signals,
};
use alloc::format;
pub use context::UserContext;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sepc, sie, stval, stvec,
};

pub static mut TIMER_INTERRUPT: usize = 0;

pub fn get_bad_addr() -> usize {
    stval::read()
}

pub fn get_bad_instruction() -> usize {
    stval::read()
}

pub fn get_exception_cause() -> TrapImpl {
    scause::read().cause()
}

global_asm!(include_str!("trap.S"));

extern "C" {
    pub fn __alltraps();
    pub fn __restore();
    pub fn __call_sigreturn();
}

pub fn init() {
    set_kernel_trap_entry();
}

fn set_kernel_trap_entry() {
    unsafe {
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }
}

fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
    }
}

pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

#[no_mangle]
pub fn trap_handler() -> ! {
    set_kernel_trap_entry();
    
    // === 添加调试打印 ===
    let raw_tp: usize;
    unsafe { core::arch::asm!("mv {}, tp", out(reg) raw_tp); }
    let scause = scause::read();

    // 安全地记录时间，仅当有任务时
    if let Some(task) = current_task() {
        let mut inner = task.acquire_inner_lock();
        inner.update_process_times_enter_trap();
    }

    let scause = scause::read();
    let stval = stval::read();
    
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            // 1. 获取系统调用 ID 和参数
            // 使用单独的块 {} 限制作用域，确保 task 变量在 syscall 之前被 Drop
            // 否则 sys_exit 挂起时，栈上会残留 task 的引用，导致引用计数无法清零
            let (syscall_id, args) = if let Some(task) = current_task() {
                let mut inner = task.acquire_inner_lock();
                let cx = inner.get_trap_cx();
                cx.gp.pc += 4; // 跳过 ecall 指令
                (
                    cx.gp.a7,
                    [cx.gp.a0, cx.gp.a1, cx.gp.a2, cx.gp.a3, cx.gp.a4, cx.gp.a5],
                )
            } else {
                 // 之前添加的 Panic 调试信息
                 let raw_tp: usize;
                 unsafe { core::arch::asm!("mv {}, tp", out(reg) raw_tp); }
                 panic!("Syscall from Idle is impossible! tp={}, cpu_id={}", raw_tp, crate::task::processor::current_cpu_id());
            }; 
            // ^^^ 关键点：在这里，'task' 变量离开作用域被 Drop，引用计数恢复正常

            // 2. 执行系统调用
            // 此时栈上不再持有当前任务的强引用
            // 如果是 sys_exit，它将不会返回，但因为 task 已被释放，wait4 可以正常回收资源
            let result = syscall(syscall_id, args);

            // 3. 处理返回值
            // 只有当 syscall 返回时（即不是 exit），才会执行到这里
            // 重新获取任务上下文写入返回值
            if let Some(task) = current_task() {
                let mut inner = task.acquire_inner_lock();
                let cx = inner.get_trap_cx();
                cx.gp.a0 = result as usize;
            }
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            if let Some(task) = current_task() {
                let mut inner = task.acquire_inner_lock();
                let addr = VirtAddr::from(stval);
                log::debug!(
                    "[page_fault] pid: {}, type: {:?}",
                    task.pid.0,
                    scause.cause()
                );
                frame_reserve(3);
                if let Err(error) = task.vm.lock().do_page_fault(addr) {
                    match error {
                        MemoryError::BeyondEOF => {
                            inner.add_signal(Signals::SIGBUS);
                        }
                        MemoryError::NoPermission | MemoryError::BadAddress => {
                            inner.add_signal(Signals::SIGSEGV);
                        }
                        _ => unreachable!(),
                    }
                };
            }
            else {
                panic!("Kernel PageFault in Idle/Init! scause: {:?}, stval: {:#x}", scause.cause(), stval);
            }
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            if let Some(task) = current_task() {
                let mut inner = task.acquire_inner_lock();
                inner.add_signal(Signals::SIGILL);
            } else {
                 panic!("IllegalInstruction in Idle!");
            }
        }
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            do_wake_expired();
            crate::fs::dev::interrupts::Interrupts::increment_interrupt_count(5);
            set_next_trigger();
            
            // 【关键修复】区分有任务和无任务(Idle)的情况
            if current_task().is_some() {
                suspend_current_and_run_next();
            } else {
                // 如果是 Idle 状态，不要走 trap_return (那会尝试切回用户态并 panic)
                // 直接回到调度循环找新任务
                run_tasks();
            }
        }
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            crate::fs::dev::interrupts::Interrupts::increment_interrupt_count(9);
            
            // 【关键修复】同上
            if current_task().is_some() {
                suspend_current_and_run_next();
            } else {
                run_tasks();
            }
        }
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }

    // 只有当有任务时，才执行从 Trap 返回到用户态的逻辑
    if let Some(task) = current_task() {
        let mut inner = task.acquire_inner_lock();
        inner.update_process_times_leave_trap(scause.cause());
        drop(inner); // 记得释放锁
        drop(task);  // 释放 task 引用
        trap_return();
    } else {
        // 如果代码走到这里且没有任务 (Idle)，说明上面没有处理好分支
        // 重新进入调度循环
        run_tasks();
        panic!("Unreachable in trap_handler: run_tasks returned!");
    }
}

#[no_mangle]
pub fn trap_return() -> ! {
    do_signal();
    set_user_trap_entry();
    // 这里的 unwrap 现在是安全的，因为我们在 trap_handler 里拦截了 None 的情况
    let task = current_task().unwrap();
    let trap_cx_ptr = task.trap_cx_user_va();
    let user_satp = task.get_user_token();
    drop(task);
    let restore_va = __restore as usize - __alltraps as usize + TRAMPOLINE;
    unsafe {
        asm!(
            "fence.i",
            "jr {restore_va}",
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_ptr,
            in("a1") user_satp,
            options(noreturn)
        );
    }
}

#[no_mangle]
pub fn trap_from_kernel() -> ! {
    use riscv::register::{sstatus, sepc};

    let scause = scause::read();
    let stval = stval::read();
    
    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            if current_task().is_some() {
                let saved_sepc = sepc::read();
                let saved_sstatus = sstatus::read();
                suspend_current_and_run_next();
                unsafe {
                    sepc::write(saved_sepc);
                    core::arch::asm!("csrw sstatus, {}", in(reg) saved_sstatus.bits());
                    core::arch::asm!("sret", options(noreturn));
                }
            } else {
                unsafe {
                    core::arch::asm!("sret", options(noreturn));
                }
            }
        }
        _ => {
            panic!(
                "a trap {:?} from kernel! bad addr = {:#x}, bad instruction = {:#x}",
                scause.cause(),
                stval,
                match current_task() {
                    Some(task) => {
                        task.acquire_inner_lock().get_trap_cx().gp.pc
                    }
                    None => {
                        sepc::read()
                    }
                }
            );
        }
    }
}