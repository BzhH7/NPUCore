#![no_std]
#![no_main]

use user_lib::{exec, exit, fork, shutdown, waitpid};

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    exit(main());
}

#[no_mangle]
fn main() -> i32 {
    let shell_path = "/bash\0";
    let script_name = "run-all.sh\0";

    let environ = [
        "SHELL=/bash\0".as_ptr(),
        "PWD=/\0".as_ptr(),
        "PATH=/:/bin\0".as_ptr(), 
        core::ptr::null(),
    ];

    let args = [
        shell_path.as_ptr(),
        script_name.as_ptr(),
        core::ptr::null(),
    ];

    let mut exit_code: i32 = 0;

    // fork 子进程来运行测试
    let pid = fork();
    if pid == 0 {
        // 子进程执行
        // 不需要 chdir，因为默认就在根目录 /
        exec(shell_path, &args, &environ);
    } else {
        // 父进程等待
        waitpid(pid as usize, &mut exit_code);
    }

    shutdown();
    0
}