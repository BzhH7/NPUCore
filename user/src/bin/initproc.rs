#![no_std]
#![no_main]

use user_lib::{exec, exit, fork, waitpid, shutdown, println}; 

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    exit(main());
}

#[no_mangle]
fn main() -> i32 {
    // 1. 设置 Shell 路径和环境变量
    let path = "/bash\0";
    let environ = [
        "SHELL=/bash\0".as_ptr(),
        "PWD=/\0".as_ptr(),
        "PATH=/\0".as_ptr(),
        "LD_LIBRARY_PATH=/\0".as_ptr(),
        core::ptr::null(),
    ];

    let tests = [
        [path.as_ptr(), "-c\0".as_ptr(), "./cr-1\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./cr-2\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./cr-3\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./cr-4\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./cr-5\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./ef2-1\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./ef2-2\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./ef2-3\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./ef2-4\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./ef2-5\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./wi-1\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./wi-2\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./wi-3\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./wi-4\0".as_ptr(), core::ptr::null()],

    ];

    let mut exit_code: i32 = 0;

    println!("[initproc] Starting standalone tests directly from root...");

    for argv in tests.iter() {
        println!("[initproc] Running test command...");

        let pid = fork();
        if pid == 0 {
            
            // 执行 bash -c "./test_name"
            exec(path, argv, &environ);
            
            println!("[initproc] Failed to exec bash");
            exit(-1);
        } else {
            // 父进程等待
            waitpid(pid as usize, &mut exit_code);
        }
    }

    println!("[initproc] All tests finished!");
    shutdown();
    0
}