#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;
// 使用 fork, waitpid, exit 替代线程操作
use user_lib::{get_time, yield_, fork, waitpid, exit};

// 建议设置 >= CPU核心数 (例如 4)
const PROCESS_NUM: usize = 4; 
// 循环次数
const LOOP_NUM: usize = 500; 

fn worker(_id: usize) {
    // 频繁让出 CPU，制造调度压力
    for _ in 0..LOOP_NUM {
        yield_(); 
    }
    // 子进程任务结束，退出
    exit(0);
}

#[no_mangle]
pub fn main() -> i32 {
    println!("[Benchmark] Starting yield test with {} processes, {} loops/process", PROCESS_NUM, LOOP_NUM);
    
    let start_time = get_time();
    let mut pids = [0isize; PROCESS_NUM];
    
    // 1. 创建并发进程
    for i in 0..PROCESS_NUM {
        let pid = fork();
        if pid == 0 {
            // 子进程执行 worker 逻辑
            worker(i);
            // worker 内部已经 exit 了，理论上不会走到这
            exit(0);
        } else {
            // 父进程记录子进程 PID
            pids[i] = pid;
        }
    }
    
    // 2. 等待所有子进程结束
    let mut exit_code: i32 = 0;
    for i in 0..PROCESS_NUM {
        // waitpid 只要传入 pid 即可等待指定子进程
        waitpid(pids[i] as usize, &mut exit_code);
    }
    
    let end_time = get_time();
    let duration_ms = end_time - start_time;
    
    println!("[Benchmark] Finished!");
    println!("[Benchmark] Total time: {} ms", duration_ms);
    
    // 计算吞吐量
    let total_yields = PROCESS_NUM * LOOP_NUM;
    if duration_ms > 0 {
        println!("[Benchmark] Throughput: {} yields/sec", (total_yields as u64 * 1000) / duration_ms as u64);
    } else {
        println!("[Benchmark] Too fast to measure throughput!");
    }
    
    0
}