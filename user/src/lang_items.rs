use super::exit;

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    // 第一层判断：是否有文件位置信息
    if let Some(location) = panic_info.location() {
        // 第二层判断：是否有 panic 消息
        if let Some(msg) = panic_info.message() {
            println!(
                "Panicked at {}:{}, {}",
                location.file(),
                location.line(),
                msg
            );
        } else {
            println!(
                "Panicked at {}:{}, (no message)",
                location.file(),
                location.line()
            );
        }
    } else {
        // 没有位置信息的情况
        if let Some(msg) = panic_info.message() {
            println!("Panicked: {}", msg);
        } else {
            println!("Panicked: (no message)");
        }
    }

    exit(-1);
}