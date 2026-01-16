#![no_std]
#![no_main]
#![feature(linkage)]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(asm_experimental_arch)]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![feature(int_roundings)]
#![feature(string_remove_matches)]
#![allow(internal_features)]
#![feature(lang_items)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![feature(const_maybe_uninit_assume_init)]
#![feature(trait_upcasting)]
#![feature(core_intrinsics)]
#![allow(dead_code)]
#![allow(unused_assignments)]
#![allow(unused_variables)]
pub use hal::config;
extern crate alloc;
extern crate core;

#[macro_use]
extern crate bitflags;

#[macro_use]
mod console;
mod drivers;
mod fs;
mod hal;
mod lang_items;
mod math;
mod mm;
mod net;
mod syscall;
mod task;
mod timer;
mod utils;

#[cfg(feature = "block_mem")]
use crate::config::DISK_IMAGE_BASE;
use crate::hal::bootstrap_init;
use crate::hal::machine_init;
#[cfg(feature = "riscv")]
use crate::hal::arch::riscv::{ap_init, ap_finish_init};
#[cfg(feature = "board_2k1000")]
core::arch::global_asm!(include_str!("hal/arch/loongarch64/entry.asm"));
#[cfg(feature = "riscv")]
core::arch::global_asm!(include_str!("hal/arch/riscv/entry.asm"));
#[cfg(all(feature = "block_mem", feature = "loongarch64"))]
core::arch::global_asm!(include_str!("load_img.S"));
#[cfg(all(feature = "block_mem", feature = "riscv"))]
core::arch::global_asm!(include_str!("load_img-rv.S"));
#[cfg(all(not(feature = "block_mem"), feature = "loongarch64"))]
core::arch::global_asm!(include_str!("preload_app.S"));
#[cfg(all(not(feature = "block_mem"), feature = "riscv"))]
core::arch::global_asm!(include_str!("preload_app-rv.S"));

fn mem_clear() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    #[cfg(feature = "zero_init")]
    unsafe {
        core::slice::from_raw_parts_mut(
            sbss as usize as *mut u8,
            crate::config::MEMORY_END - sbss as usize,
        )
        .fill(0);
    }
    #[cfg(not(feature = "zero_init"))]
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

// 这一行可能有误，需要后续处理
#[cfg(feature = "block_mem")]
fn move_to_high_address() {
    extern "C" {
        fn simg();
        fn eimg();
    }
    unsafe {
        // 加载根文件系统镜像
        let img =
            core::slice::from_raw_parts(simg as usize as *mut u8, eimg as usize - simg as usize);
        // 以DISK_IMAGE_BASE到MEMORY_END上的内存作为根文件系统镜像
        #[cfg(all(feature = "block_mem", feature = "riscv"))]
        let mem_disk = core::slice::from_raw_parts_mut(
            DISK_IMAGE_BASE as *mut u8,
            // 大小为128MB
            0x1000_0000,
        );
        #[cfg(all(feature = "block_mem", feature = "loongarch64"))]
        let mem_disk = core::slice::from_raw_parts_mut(
            DISK_IMAGE_BASE as *mut u8,                     
            // 大小为64MB
            0x800_0000,
        );
        // 清空mem_disk上的内容
        mem_disk.fill(0);
        // 将img上的所有内容copy到mem_disk上，可能是因为这一步
        // 所以img大小不得大于64MB
        mem_disk[..img.len()].copy_from_slice(img);
    }
}

use core::sync::atomic::{AtomicBool, Ordering};
use core::hint::spin_loop;

#[link_section = ".data"]
static AP_CAN_START: AtomicBool = AtomicBool::new(false);
#[link_section = ".data"] 
static BOOT_FLAG: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "riscv")]
// 引入 sbi 模块
use crate::hal::arch::riscv::sbi;
use crate::config::MAX_CPU_NUM;

// 声明汇编入口 _start，我们需要它的地址
extern "C" {
    fn _start();
}

use crate::hal::TrapContext;

#[no_mangle]
pub fn rust_main(hart_id: usize) -> ! {
    
    #[cfg(target_arch = "riscv64")]
    unsafe {
        riscv::register::sstatus::clear_sie();
    }

    // 1. 判断是否为 BSP (原子操作 CAS)
    // 只有第一个执行这行代码的核心会得到 is_bsp = true
    let is_bsp = !BOOT_FLAG.swap(true, Ordering::SeqCst);

    // ⚠️ 关键修复：BSP 和 AP 的初始化路径分离
    // BSP: 完整初始化（trap vector + timer interrupt）
    // AP: 只设置 trap vector，不启用 timer interrupt（避免在初始化完成前触发中断）
    #[cfg(feature = "riscv")]
    if is_bsp {
        machine_init();  // BSP: 设置 trap vector 并启用 timer interrupt
    } else {
        ap_init();       // AP: 只设置 trap vector，不启用 timer interrupt
    }

    // ⚠️ 注意：在锁初始化之前，尽量不要多核同时 Println，否则还是会乱。
    // 这里我们先不打印，等 Console 初始化好后再打印。

    if is_bsp {
        // ==========================
        //       主核 (BSP) 逻辑
        // ==========================
        
        // 清空 BSS (必须最先做，且只能做一次)
        mem_clear();
        
        // 初始化串口和 Console (这里面应该包含锁的初始化)
        console::log_init(); 
        
        // 此时 Println 应该是安全的了
        println!("[kernel] Console initialized by BSP.");
        println!("[Boot] Hart {} is BSP, starting initialization...", hart_id);

        bootstrap_init();

        #[cfg(all(feature = "block_mem"))]
        move_to_high_address();

        mm::init(); // 初始化堆
        println!("[kernel] Heap initialized.");

        // 初始化其他子系统...
        fs::directory_tree::init_fs();
        
        println!("[Debug] Calling net::config::init()...");
        net::config::init();
        println!("[Debug] net::config::init() done.");

        #[cfg(feature = "block_virt")]
        println!("[kernel] block in virt mode!");
        
        #[cfg(feature = "oom_handler")]
        println!("[kernel] oom_handler is enabled!");

        #[cfg(any(feature = "block_virt_pci", feature = "block_virt"))]
        {
            println!("[Debug] Calling fs::flush_preload()..."); 
            fs::flush_preload();
            println!("[Debug] fs::flush_preload() done.");
        }

        println!("[kernel] Loading initproc... (before call)");
        task::add_initproc();
        println!("[kernel] Initproc loaded! (after call)");

        // ------------------------------------------
        //         唤醒从核 (Secondary Harts)
        // ------------------------------------------
        let start_vaddr = _start as usize;
        // 如果开启了分页，需要把虚拟地址转为物理地址给 SBI
        // 假设有一个宏或函数做这个转换，或者直接用物理地址启动
        // 这里沿用你原来的逻辑
        let start_paddr = start_vaddr & !0xffffffff00000000; 

        println!("[Boot] BSP is waking up secondary harts...");

        for i in 0..MAX_CPU_NUM {
            if i == hart_id { continue; } // 跳过自己

            #[cfg(feature = "riscv")]
            {
                // 唤醒目标核
                let ret = sbi::hart_start(i, start_paddr, 0);
                if ret == 0 {
                    println!("[Boot] Hart {} started command sent.", i);
                } else {
                    println!("[Boot] Failed to start Hart {} (error: {}).", i, ret);
                }
            }
        }

        // ⚠️ 关键修复：强制初始化所有 lazy_static 全局变量
        // 在 AP 启动前完成初始化，防止多核竞争初始化导致的死锁
        task::init_task_subsystem();
        println!("[Boot] Global task structures initialized.");

        // 通知从核可以继续执行了
        // Release 保证之前的内存写入（如页表、内核栈初始化）对 Acquire 的从核可见
        AP_CAN_START.store(true, Ordering::Release);
        println!("[Boot] BSP barrier released. All harts enter main loop.");

    } else {
        // ==========================
        //       从核 (AP) 逻辑
        // ==========================
        
        // ⚠️ 关键修改：移除这里的 sbi::console_putchar
        // 原因：此时 BSP 正在疯狂输出初始化日志，AP 如果插嘴，屏幕就会乱码。
        // AP 应该保持“静默”，直到收到出发信号。

        while !AP_CAN_START.load(Ordering::Acquire) {
            spin_loop(); // CPU 提示，降低功耗
        }
        
        // ⚠️ 关键修复：AP 必须激活内核页表！
        // 否则 AP 的 satp=0（无分页），无法正常执行内核代码
        mm::KERNEL_SPACE.lock().activate();
        
        // ⚠️ 关键修复：AP 在同步屏障后才启用 timer interrupt
        // 此时 BSP 已完成所有初始化，可以安全启用中断
        #[cfg(feature = "riscv")]
        ap_finish_init();
        
        // 此时 BSP 已经初始化完锁和全局资源，可以安全打印了
        println!("[Boot] Hart {} (AP) implies ready and running.", hart_id);
    }

    // ==========================
    //     所有核心通用逻辑
    // ==========================
    
    #[cfg(target_arch = "riscv64")]
    unsafe { riscv::register::sstatus::set_sie(); }

    // 进入调度循环
    println!("[kernel] Hart {} entering task loop...", hart_id);
    task::run_tasks();
    
    panic!("Unreachable in rust_main!");
}

#[cfg(test)]
fn test_runner(_tests: &[&dyn Fn()]) {}
