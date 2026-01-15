use crate::hal::{console_flush, console_putchar, disable_interrupts, restore_interrupts};
use crate::task::current_task;
use core::fmt::{self, Write};
use log::{self, Level, LevelFilter, Log, Metadata, Record};
use spin::Mutex;

/// Kernel output writer for console
struct KernelOutput;

impl Write for KernelOutput {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        const FLUSH_THRESHOLD: usize = 4;
        let mut count = 0;
        for c in s.chars() {
            console_putchar(c as usize);
            count += 1;
            if count >= FLUSH_THRESHOLD {
                console_flush();
                count = 0;
            }
        }
        if count != 0 {
            console_flush();
        }
        Ok(())
    }
}

/// Global stdout with spinlock protection
static STDOUT: Mutex<KernelOutput> = Mutex::new(KernelOutput);

/// Print formatted output to console with interrupt protection
pub fn print(args: fmt::Arguments) {
    // Disable interrupts before acquiring lock to prevent deadlock from timer interrupt
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
