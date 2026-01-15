use crate::hal::{console_flush, console_putchar};
use crate::task::current_task;
use core::fmt::{self, Write};
use log::{self, Level, LevelFilter, Log, Metadata, Record};
use spin::Mutex;
#[cfg(feature = "riscv")]
use riscv::register::sstatus;
#[cfg(feature = "loongarch64")]
use crate::hal::arch::loongarch64::CrMd;

struct KernelOutput;

impl Write for KernelOutput {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut i = 0;
        for c in s.chars() {
            console_putchar(c as usize);
            i += 1;
            if i >= 4 {
                console_flush();
                i = 0;
            }
        }
        if i != 0 {
            console_flush();
        }
        Ok(())
    }
}

// 2. 使用 Mutex 包裹 KernelOutput
// spin::Mutex 在 no_std 下就是自旋锁，遇到锁时会忙等待
static STDOUT: Mutex<KernelOutput> = Mutex::new(KernelOutput);

pub fn print(args: fmt::Arguments) {
    // 【修复】：在获取锁之前关闭中断，防止在持有锁时被时钟中断抢占导致死锁
    let interrupts_were_enabled = disable_interrupts();

    STDOUT.lock().write_fmt(args).unwrap();

    restore_interrupts(interrupts_were_enabled);
}

// 下面的宏定义不用动，它们会调用上面的 print 函数
#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?))
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, crate::newline!()) $(, $($arg)+)?))
    }
}

pub fn log_init() {
    static LOGGER: Logger = Logger;
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(match option_env!("LOG") {
        Some("error") => LevelFilter::Error,
        Some("warn") => LevelFilter::Warn,
        Some("info") => LevelFilter::Info,
        Some("debug") => LevelFilter::Debug,
        Some("trace") => LevelFilter::Trace,
        _ => LevelFilter::Off,
    });
}

struct Logger;
impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        print!("\x1b[{}m", level_to_color_code(record.level()));
        match current_task() {
            Some(task) => println!("pid {}: {}", task.pid.0, record.args()),
            None => println!("kernel: {}", record.args()),
        }
        print!("\x1b[0m")
    }

    fn flush(&self) {}
}

fn level_to_color_code(level: Level) -> u8 {
    match level {
        Level::Error => 31, // Red
        Level::Warn => 93,  // BrightYellow
        Level::Info => 34,  // Blue
        Level::Debug => 32, // Green
        Level::Trace => 90, // BrightBlack
    }
}

#[cfg(feature = "riscv")]
fn disable_interrupts() -> bool {
    let sie = sstatus::read().sie();
    if sie {
        unsafe { sstatus::clear_sie(); }
    }
    sie
}

#[cfg(feature = "riscv")]
fn restore_interrupts(was_enabled: bool) {
    if was_enabled {
        unsafe { sstatus::set_sie(); }
    }
}

#[cfg(feature = "loongarch64")]
fn disable_interrupts() -> bool {
    let mut crmd = CrMd::read();
    let was_enabled = crmd.is_interrupt_enabled();
    if was_enabled {
        crmd.set_ie(false).write();
    }
    was_enabled
}

#[cfg(feature = "loongarch64")]
fn restore_interrupts(was_enabled: bool) {
    if was_enabled {
        CrMd::read().set_ie(true).write();
    }
}

#[cfg(not(any(feature = "riscv", feature = "loongarch64")))]
fn disable_interrupts() -> bool {
    false
}

#[cfg(not(any(feature = "riscv", feature = "loongarch64")))]
fn restore_interrupts(_was_enabled: bool) {}
