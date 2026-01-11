use crate::hal::shutdown;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        println!(
            "[kernel] panicked at '{}', {}:{}:{}",
            // 修复：直接在 println! 内部处理默认消息，避免临时变量被过早释放
            info.message().unwrap_or(&core::format_args!("no message")),
            location.file(),
            location.line(),
            location.column()
        );
    } else {
        println!(
            "[kernel] panicked at '{}'",
            info.message().unwrap_or(&core::format_args!("no message"))
        );
    }
    shutdown()
}

pub trait Bytes<T> {
    fn as_bytes(&self) -> &[u8] {
        let size = core::mem::size_of::<T>();
        unsafe {
            core::slice::from_raw_parts(self as *const _ as *const T as usize as *const u8, size)
        }
    }

    fn as_bytes_mut(&mut self) -> &mut [u8] {
        let size = core::mem::size_of::<T>();
        unsafe {
            core::slice::from_raw_parts_mut(self as *mut _ as *mut T as usize as *mut u8, size)
        }
    }
}