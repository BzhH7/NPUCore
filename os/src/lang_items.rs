use crate::hal::shutdown;
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