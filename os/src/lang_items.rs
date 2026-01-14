use crate::hal::shutdown;
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