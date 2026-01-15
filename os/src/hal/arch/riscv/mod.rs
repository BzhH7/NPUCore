pub mod config;
pub mod kern_stack;
pub mod sbi;
pub mod sv39;
pub mod switch;
pub mod time;
pub mod trap;

#[cfg(feature = "board_rvqemu")]
#[path = "../../platform/riscv/qemu.rs"]
pub mod rv_board;

#[cfg(feature = "board_visionfive2")]
#[path = "../../platform/riscv/visionfive2.rs"]
pub mod rv_board;

pub fn machine_init() {
    trap::init();
    trap::enable_timer_interrupt();
    set_next_trigger();
}

use time::set_next_trigger;

pub use trap::context::MachineContext;

pub type KernelPageTableImpl = sv39::Sv39PageTable;
pub type PageTableImpl = sv39::Sv39PageTable;
pub type TrapImpl = riscv::register::scause::Trap;
pub type InterruptImpl = riscv::register::scause::Interrupt;
pub type ExceptionImpl = riscv::register::scause::Exception;

pub fn bootstrap_init() {}

pub fn boot_entry_paddr(entry_vaddr: usize) -> usize {
    entry_vaddr & !0xffffffff00000000
}

pub fn disable_interrupts() -> bool {
    let sie = riscv::register::sstatus::read().sie();
    if sie {
        unsafe { riscv::register::sstatus::clear_sie(); }
    }
    sie
}

pub fn restore_interrupts(was_enabled: bool) {
    if was_enabled {
        unsafe { riscv::register::sstatus::set_sie(); }
    }
}
