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
        "cr-1\0", "cr-2\0", "cr-3\0", "cr-4\0", "cr-5\0", "ef2-1\0", "ef2-2\0", 
        "ef2-3\0", "ef2-4\0", "ef2-5\0", "wi-1\0", "wi-2\0", "wi-3\0", "wi-4\0"
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