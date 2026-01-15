//! ELF (Executable and Linkable Format) parsing
//!
//! This module handles:
//! - ELF file parsing and loading
//! - Program header processing
//! - Auxiliary vector construction
//! - Dynamic linker support

/*
    此文件用于解析ELF文件
    内容与RISCV版本相同，无需修改
*/
use alloc::boxed::Box;

use crate::{
    fs::{OpenFlags, ROOT_FD},
    mm::{Frame, KERNEL_SPACE},
    syscall::errno::*,
};

/// Auxiliary vector types
///
/// Used to pass information from kernel to user program at startup
#[derive(Clone, Copy)]
#[allow(non_camel_case_types, unused)]
#[repr(usize)]
pub enum AuxvType {
    /// End of vector
    NULL = 0,
    /// Entry to ignore
    IGNORE = 1,
    /// File descriptor of program
    EXECFD = 2,
    /// Program headers for program
    PHDR = 3,
    /// Size of program header entry
    PHENT = 4,
    /// Number of program headers
    PHNUM = 5,
    /// System page size
    PAGESZ = 6,
    /// Base address of interpreter
    BASE = 7,
    /// Flags
    FLAGS = 8,
    /// Entry point of program
    ENTRY = 9,
    /// Program is not ELF
    NOTELF = 10,
    /// Real user ID
    UID = 11,
    /// Effective user ID
    EUID = 12,
    /// Real group ID
    GID = 13,
    /// Effective group ID
    EGID = 14,
    /// Platform string
    PLATFORM = 15,
    /// Hardware capabilities
    HWCAP = 16,
    /// Clock tick
    CLKTCK = 17,
    /// FPU control word
    FPUCW = 18,
    /// Data cache block size
    DCACHEBSIZE = 19,
    /// Instruction cache block size
    ICACHEBSIZE = 20,
    /// Unified cache block size
    UCACHEBSIZE = 21,
    /// Ignore PowerPC entry
    IGNOREPPC = 22,
    /// Secure mode boolean
    SECURE = 23,
    /// Base platform string
    BASE_PLATFORM = 24,
    /// Random bytes address
    RANDOM = 25,
    /// Extended hardware capabilities
    HWCAP2 = 26,
    /// Filename of program
    EXECFN = 31,
    /// Sysinfo address
    SYSINFO = 32,
    /// Sysinfo EHDR address
    SYSINFO_EHDR = 33,
    /// L1 instruction cache shape
    L1I_CACHESHAPE = 34,
    /// L1 data cache shape
    L1D_CACHESHAPE = 35,
    /// L2 cache shape
    L2_CACHESHAPE = 36,
    /// L3 cache shape
    L3_CACHESHAPE = 37,
    L1I_CACHESIZE = 40,
    L1I_CACHEGEOMETRY = 41,
    L1D_CACHESIZE = 42,
    L1D_CACHEGEOMETRY = 43,
    L2_CACHESIZE = 44,
    L2_CACHEGEOMETRY = 45,
    L3_CACHESIZE = 46,
    L3_CACHEGEOMETRY = 47,
    MINSIGSTKSZ = 51,
}

#[derive(Clone, Copy)]
#[allow(unused)]
#[repr(C)]
pub struct AuxvEntry {
    auxv_type: AuxvType,
    auxv_val: usize,
}

impl AuxvEntry {
    pub fn new(auxv_type: AuxvType, auxv_val: usize) -> Self {
        Self {
            auxv_type,
            auxv_val,
        }
    }
}

#[repr(C)]
pub struct ELFInfo {
    // 入口地址
    pub entry: usize,
    // 解析器入口地址
    pub interp_entry: Option<usize>,
    // 基地址
    pub base: usize,
    // 程序头表条目数量
    pub phnum: usize,
    // 程序头表条目大小
    pub phent: usize,
    // 程序头表地址
    pub phdr: usize,
}

/// 加载ELF解释器
pub fn load_elf_interp(path: &str) -> Result<&'static [u8], isize> {
    // 只读方式打开指定path的文件
    match ROOT_FD.open(path, OpenFlags::O_RDONLY, false) {
        Ok(file) => {
            // 文件大小小于ELF文件头大小
            if file.get_size() < 4 {
                return Err(ELIBBAD);
            }
            // 读取文件头的前4个字节，即魔数'\x7fELF'
            let mut magic_number = Box::<[u8; 4]>::new([0; 4]);
            // this operation may be expensive... I'm not sure
            // 原作者注释：这个操作可能很昂贵...我不确定
            file.read(Some(&mut 0usize), magic_number.as_mut_slice());
            // 匹配魔数
            match magic_number.as_slice() {
                // 正确情况
                b"\x7fELF" => {
                    // 获取内核空间的最高地址
                    let buffer_addr = KERNEL_SPACE.lock().highest_addr();
                    // 在内核空间的最高地址来分配一个缓冲区
                    let buffer = unsafe {
                        core::slice::from_raw_parts_mut(buffer_addr.0 as *mut u8, file.get_size())
                    };
                    // 获取文件的所有缓存
                    let caches = file.get_all_caches().unwrap();
                    // 将缓存内容映射到frame中
                    let frames = caches
                        .iter()
                        .map(|cache| Frame::InMemory(cache.try_lock().unwrap().get_tracker()))
                        .collect();

                    // 将文件内容映射到内核空间
                    crate::mm::KERNEL_SPACE
                        .lock()
                        .insert_program_area(
                            buffer_addr.into(),
                            crate::mm::MapPermission::R | crate::mm::MapPermission::W,
                            frames,
                        )
                        .unwrap();

                    return Ok(buffer);
                }
                // 不是ELF文件
                _ => Err(ELIBBAD),
            }
        }
        Err(errno) => Err(errno),
    }
}
