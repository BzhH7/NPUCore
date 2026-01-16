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
    pub fn __kernelvec(); // 引入新的汇编入口
}

pub fn init() {
    set_kernel_trap_entry();

    // 我们使用 SBI 轮询 (console_getchar) 来读取输入，不需要处理 PLIC 中断。
    // 如果开启而不处理 (Claim/Complete)，会导致中断风暴卡死系统。
    unsafe {
        riscv::register::sie::clear_sext();
    }
}

fn set_kernel_trap_entry() {
    unsafe {
        // 修改：使用 __kernelvec 作为内核陷阱入口
        stvec::write(__kernelvec as usize, TrapMode::Direct);
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
                let addr = VirtAddr::from(stval);
                log::debug!(
                    "[page_fault] pid: {}, type: {:?}",
                    task.pid.0,
                    scause.cause()
                );
                // 关键修复：先处理内存映射（持有 vm lock），再处理信号（持有 inner lock）
                // 避免锁嵌套导致的死锁
                frame_reserve(3);
                let page_fault_result = {
                    task.vm.lock().do_page_fault(addr)
                };
                
                if let Err(error) = page_fault_result {
                    let mut inner = task.acquire_inner_lock();
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

            if unsafe { TIMER_INTERRUPT } % 100 == 0 {
                log::trace!("[Trap] Timer interrupt triggered");
            }

            do_wake_expired();
            crate::fs::dev::interrupts::Interrupts::increment_interrupt_count(5);
            set_next_trigger();
            
            // 【关键修复】区分有任务和无任务(Idle)的情况
            if current_task().is_some() {
                suspend_current_and_run_next();
                // Debug: verify task is still current after resume
                if current_task().is_none() {
                    panic!("[trap_handler] current_task is None after suspend_current_and_run_next!");
                }
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
    
    // ⚠️ 关键修复：在返回用户态前重新设置定时器
    // 这样可以清除可能已经 pending 的定时器中断
    // 特别重要：当任务从一个 CPU 迁移到另一个 CPU 时，
    // 目标 CPU 的定时器可能已经在之前设置并 pending
    set_next_trigger();
    
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
static mut TICKS: usize = 0;
#[no_mangle]
pub fn trap_from_kernel() {
    use riscv::register::{sstatus, sepc};

    // === 读取 tp 和 sp ===
    let raw_tp: usize;
    let raw_sp: usize;
    let raw_ra: usize;
    unsafe { 
        core::arch::asm!("mv {}, tp", out(reg) raw_tp);
        core::arch::asm!("mv {}, sp", out(reg) raw_sp);
        core::arch::asm!("mv {}, ra", out(reg) raw_ra);
    }
    // ==============

    let scause = scause::read();
    let stval = stval::read();
    // 获取真正的内核崩溃地址
    let kernel_pc = sepc::read();
    
    // Debug: Check if sepc is 0 or invalid when entering kernel trap
    if kernel_pc == 0 {
        panic!("[KTRAP] sepc=0 on entry! TP={} SP={:#x} RA={:#x} cause={:?}", 
               raw_tp, raw_sp, raw_ra, scause.cause());
    }
    
    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            do_wake_expired(); 

            // === 【诊断代码】每 100 次时钟中断打印一个点 ===
            unsafe {
                TICKS += 1;
                if TICKS % 100 == 0 {
                    // 只让主核打印，避免输出混乱
                    if crate::task::processor::current_cpu_id() == 0 {
                        print!(".");
                    }
                }
            }
            // =============================================

            // 【调试】暂时禁用内核态时钟中断的调度
            // 内核态的时钟中断不调度，只更新时间并返回
            // 这样可以排查是否是调度导致的问题
            /*
            if current_task().is_some() {
                suspend_current_and_run_next();
                
                // Debug: Check ra after resuming from suspend
                let ra_after: usize;
                unsafe { core::arch::asm!("mv {}, ra", out(reg) ra_after); }
                if ra_after == 0 || ra_after < 0x80000000 {
                    panic!("[KTRAP-TIMER] Invalid ra={:#x} after suspend!", ra_after);
                }
            }
            */
        }
        // 【修复】：添加对内核态外部中断的处理
        // 防止 UART 中断打断内核执行时导致 Panic
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            // 这里可以选择忽略，或者像 trap_handler 那样统计计数
            // 如果使用 PLIC，应该在这里 claim/complete，但目前由于你是轮询模式，
            // 收到这个中断说明中断屏蔽没做好，或者 OpenSBI 转发了中断。
            // 最安全的做法是什么都不做，直接返回，或者让出 CPU。
            
            // 简单的防 Panic 处理：
            crate::fs::dev::interrupts::Interrupts::increment_interrupt_count(9);
            
            // 甚至可以选择让出 CPU（如果是在等待输入的循环中被中断）
            // if current_task().is_some() {
            //     suspend_current_and_run_next();
            // }
        }
        _ => {
            println!("PANIC: {:?} at {:#x}", scause.cause(), kernel_pc);
            println!("  BadAddr={:#x} TP={} SP={:#x} RA={:#x}", stval, raw_tp, raw_sp, raw_ra);
            panic!("Kernel trap");
        }
    }
}