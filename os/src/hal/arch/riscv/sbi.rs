#![allow(unused)]

use core::arch::asm;

const SBI_SET_TIMER: usize = 0;
const SBI_CONSOLE_PUTCHAR: usize = 1;
const SBI_CONSOLE_GETCHAR: usize = 2;
const SBI_CLEAR_IPI: usize = 3;
const SBI_SEND_IPI: usize = 4;
const SBI_REMOTE_FENCE_I: usize = 5;
const SBI_REMOTE_SFENCE_VMA: usize = 6;
const SBI_REMOTE_SFENCE_VMA_ASID: usize = 7;
const SBI_SHUTDOWN: usize = 8;

#[inline(always)]
/// `ecall` wrapper to switch trap into S level.
fn sbi_call(which: usize, arg0: usize, arg1: usize, arg2: usize) -> usize {
    let mut ret;
    unsafe {
        asm!(
            "ecall",
            inlateout("x10") arg0 => ret,
            in("x11") arg1,
            in("x12") arg2,
            in("x17") which,
        );
    }
    ret
}

pub fn set_timer(timer: usize) {
    sbi_call(SBI_SET_TIMER, timer, 0, 0);
}

pub fn console_putchar(c: usize) {
    sbi_call(SBI_CONSOLE_PUTCHAR, c, 0, 0);
}

pub fn console_getchar() -> usize {
    sbi_call(SBI_CONSOLE_GETCHAR, 0, 0, 0)
}

pub fn console_flush() {}

pub fn shutdown() -> ! {
    sbi_call(SBI_SHUTDOWN, 0, 0, 0);
    panic!("It should shutdown!");
}

// ================= 新增：HSM 扩展 (用于多核启动) =================

const SBI_EXT_HSM: usize = 0x48534D;
const SBI_FID_HART_START: usize = 0;

/// 启动指定的核心
/// hartid: 目标核 ID
/// start_addr: 目标核启动后跳转的物理地址
/// opaque: 传递给目标核的参数 (a1 寄存器)
pub fn hart_start(hartid: usize, start_addr: usize, opaque: usize) -> usize {
    let mut ret;
    unsafe {
        asm!(
            "ecall",
            inlateout("x10") hartid => ret, // a0: 输入 hartid，输出返回值(error code)
            in("x11") start_addr,           // a1: 输入 start_addr
            in("x12") opaque,               // a2: 输入 opaque
            in("x17") SBI_EXT_HSM,          // a7: Extension ID (HSM)
            in("x16") SBI_FID_HART_START,   // a6: Function ID (hart_start)
        );
    }
    ret
}

