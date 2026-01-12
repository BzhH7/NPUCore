use crate::fs::ext4::BLOCK_SIZE;
use alloc::sync::Arc;
use core::ops::AddAssign;
use lazy_static::*;
use spin::Mutex;

use crate::drivers::BLOCK_DEVICE;

#[allow(unused, non_camel_case_types)]
#[derive(Debug)]
pub enum FS_Type {
    Null,
    Fat32,
    Ext4,
}

#[derive(Debug)]
pub struct FileSystem {
    pub fs_id: usize,
    pub fs_type: FS_Type,
}

lazy_static! {
    static ref FS_ID_COUNTER: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
}

impl FileSystem {
    pub fn new(fs_type: FS_Type) -> Self {
        FS_ID_COUNTER.lock().add_assign(1);
        let fs_id = *FS_ID_COUNTER.lock();
        Self { fs_id, fs_type }
    }
}

pub fn pre_mount() -> FS_Type {
    // 获取块设备
    let block_device = BLOCK_DEVICE.clone();
    let mut buf = [0u8; BLOCK_SIZE];

    // 1. 判断是否为 FAT32
    // 读取第 0 块
    block_device.read_block(0, &mut buf);
    // 判断第 510 和 511 字节是否为 0x55AA
    if buf[510] == 0x55 && buf[511] == 0xAA {
        println!("[fs] found fat32 filesystem");
        return FS_Type::Fat32;
    }

    // 2. 判断是否为 Ext4
    // Ext4 超级块位于磁盘偏移 1024 字节处
    // 魔数位于超级块偏移 0x38 (56) 字节处 -> 总偏移 1024 + 56 = 1080
    let magic_offset_global = 1080;
    
    // 计算所在的块号和块内偏移
    let ext4_block_id = magic_offset_global / BLOCK_SIZE;
    let ext4_offset_in_block = magic_offset_global % BLOCK_SIZE;

    // 如果 Ext4 魔数不在第 0 块（例如块大小为 512 时，它在第 2 块），需要重新读取
    if ext4_block_id != 0 {
        block_device.read_block(ext4_block_id, &mut buf);
    }

    // 读取魔数
    let magic_number = u16::from_le_bytes([
        buf[ext4_offset_in_block], 
        buf[ext4_offset_in_block + 1]
    ]);
    
    println!("[fs] read magic number: {:#x}", magic_number); // 使用十六进制打印更直观
    if magic_number == 0xEF53 {
        println!("[fs] found ext4 filesystem");
        return FS_Type::Ext4;
    }

    println!("[fs] no filesystem found");
    FS_Type::Null
}