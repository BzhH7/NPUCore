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
        [path.as_ptr(), "-c\0".as_ptr(), "./brk\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./chdir\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./clone\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./close\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./dup\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./dup2\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./execve\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./exit\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./fork\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./fstat\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./getcwd\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./getdents\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./getpid\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./getppid\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./gettimeofday\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./mkdir_\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./mmap\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./mount\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./munmap\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./open\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./openat\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./pipe\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./read\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./sleep\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./statx\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./times\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./umount\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./uname\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./unlink\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./wait\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./waitpid\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./write\0".as_ptr(), core::ptr::null()],
        [path.as_ptr(), "-c\0".as_ptr(), "./yield\0".as_ptr(), core::ptr::null()],
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