use super::exit;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let msg_str = match info.message() {
        Some(msg) => match msg.as_str() {
            Some(s) => s,
            None => "(panic message)",
        },
        None => "(panic message)",
    };

    if let Some(location) = info.location() {
        println!(
            "[kernel] panicked at {}:{}:{}: {}",
            location.file(),
            location.line(),
            location.column(),
            msg_str
        );
    } else {
        println!(
            "[kernel] panicked: {}",
            msg_str
        );
    }

    exit(-1);
}