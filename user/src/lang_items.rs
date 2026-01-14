use super::exit;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let msg = info.message();

    let msg_str = match msg.as_str() {
        Some(s) => s,
        None => "(no message)",
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