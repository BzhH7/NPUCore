#![no_std]
#![no_main]

use user_lib::{exec, exit, fork, shutdown, waitpid, println};

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    exit(main());
}

#[no_mangle]
fn main() -> i32 {
    let tests = [
        "brk\0", "chdir\0", "clone\0", "close\0", "dup\0", "dup2\0", 
        "execve\0", "exit\0", "fork\0", "fstat\0", "getcwd\0", "getdents\0", 
        "getpid\0", "getppid\0", "gettimeofday\0", "mkdir_\0", "mmap\0", 
        "mount\0", "munmap\0", "open\0", "openat\0", "pipe\0", "read\0", 
        "sleep\0", "times\0", "umount\0", "uname\0", "unlink\0", "wait\0", 
        "waitpid\0", "write\0", "yield\0" 
    ];

    // 设置 PATH 环境变量，确保能在根目录找到这些程序
    let environ = [
        "PATH=/\0".as_ptr(),
        core::ptr::null(),
    ];

    let mut exit_code: i32 = 0;

    println!("[initproc] Starting all tests...");

    for &test_name in tests.iter() {
        println!("[initproc] Running test: {}", test_name);
        
        let pid = fork();
        if pid == 0 {
            // 子进程：执行具体的测试程序
            let args = [
                test_name.as_ptr(),
                core::ptr::null(),
            ];
            
            // 执行程序
            exec(test_name, &args, &environ);
            
            // 如果 exec 失败（比如文件不存在），需要手动退出，防止子进程跑飞
            println!("[initproc] Failed to exec {}", test_name);
            exit(-1);
        } else {
            // 父进程：等待当前测试结束
            waitpid(pid as usize, &mut exit_code);
        }
    }

    println!("[initproc] All tests finished!");
    shutdown();
    0
}