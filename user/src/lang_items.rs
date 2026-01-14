use super::exit;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        match info.message() {
            Some(args) => {
                println!(
                    "[kernel] panicked at '{}', {}:{}:{}",
                    args,
                    location.file(),
                    location.line(),
                    location.column()
                );
            }
            None => {
                println!(
                    "[kernel] panicked at '(no message)', {}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                );
            }
        }
    } else {
        match info.message() {
            Some(args) => {
                println!("[kernel] panicked at '{}'", args);
            }
            None => {
                println!("[kernel] panicked at '(no message)'");
            }
        }
    }

    exit(-1)
}