use super::exit;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        println!(
            "[kernel] panicked at '{}', {}:{}:{}",
            info.message(),
            location.file(),
            location.line(),
            location.column()
        );
    } else {
        println!(
            "[kernel] panicked at '{}'",
            info.message()
        );
    }

    exit(-1)
}