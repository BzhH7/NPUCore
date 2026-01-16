//! Inode (index node) abstraction
//!
//! Provides the low-level interface for filesystem inodes including:
//! - File metadata and attributes
//! - Block-level read/write operations
//! - Directory entry management
//! - Cache management

use crate::fs::*;
use core::any::Any;

use crate::fs::fat32::layout::FATDiskInodeType;
use crate::fs::vfs::VFS;
use alloc::sync::Arc;
use alloc::vec::Vec;
use downcast_rs::*;
use fat32::fat_inode::FileContent;
use fat32::layout::FATShortDirEnt;
use spin::{Mutex, MutexGuard, RwLockReadGuard, RwLockWriteGuard};
#[allow(unused)]
use vfs::VFSDirEnt;

/// Inode lock marker type
pub struct InodeLock;

/// Inode trait for filesystem operations
///
/// Provides low-level operations on inodes including locking,
/// reading/writing at block level, and directory operations
#[allow(unused)]
pub trait InodeTrait: DowncastSync {
    /// Cast to Any for downcasting
    fn as_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
    
    /// Acquire read lock
    fn read(&self) -> RwLockReadGuard<InodeLock>;
    
    /// Acquire write lock
    fn write(&self) -> RwLockWriteGuard<InodeLock>;
    
    /// Get file type with lock
    fn get_file_type_lock(&self) -> MutexGuard<DiskInodeType>;
    
    /// Get file type
    fn get_file_type(&self) -> DiskInodeType;
    
    /// Get file size
    fn get_file_size(&self) -> u32;
    
    /// Get file size with read lock
    fn get_file_size_rlock(&self, _inode_lock: &RwLockReadGuard<InodeLock>) -> u32;
    
    /// Get file size with write lock
    fn get_file_size_wlock(&self, _inode_lock: &RwLockWriteGuard<InodeLock>) -> u32;
    
    /// Check if inode is a directory
    fn is_dir(&self) -> bool;
    
    /// Check if inode is a regular file
    fn is_file(&self) -> bool;
    
    /// Get inode number with lock
    fn get_inode_num_lock(&self, lock: &RwLockReadGuard<FileContent>) -> Option<u32>;
    
    /// Get block ID with lock
    fn get_block_id(&self, lock: &RwLockReadGuard<FileContent>, inner_cache_id: u32)
        -> Option<u32>;
    
    /// Read at block cache with read lock
    fn read_at_block_cache_rlock(
        &self,
        _inode_lock: &RwLockReadGuard<InodeLock>,
        offset: usize,
        buf: &mut [u8],
    ) -> usize;
    
    /// Read at block cache with write lock
    fn read_at_block_cache_wlock(
        &self,
        _inode_lock: &RwLockWriteGuard<InodeLock>,
        offset: usize,
        buf: &mut [u8],
    ) -> usize;
    
    /// Read at block cache
    fn read_at_block_cache(&self, offset: usize, buf: &mut [u8]) -> usize;
    
    /// Write at block cache with lock
    fn write_at_block_cache_lock(
        &self,
        inode_lock: &RwLockWriteGuard<InodeLock>,
        offset: usize,
        buf: &[u8],
    ) -> usize;
    
    /// Write at block cache
    fn write_at_block_cache(&self, offset: usize, buf: &[u8]) -> usize;
    
    /// Get single cache page
    fn get_single_cache(&self, inner_cache_id: usize) -> Arc<Mutex<PageCache>>;
    
    /// Get single cache page with lock
    fn get_single_cache_lock(
        &self,
        _inode_lock: &RwLockReadGuard<InodeLock>,
        inner_cache_id: usize,
    ) -> Arc<Mutex<PageCache>>;
    
    /// Get all cache pages
    fn get_all_cache(&self) -> Vec<Arc<Mutex<PageCache>>>;
    
    /// Get all files with lock (for directories)
    fn get_all_files_lock(
        &self,
        inode_lock: &RwLockWriteGuard<InodeLock>,
    ) -> Vec<(String, FATShortDirEnt, u32)>;
    
    /// Get directory entry information with lock
    fn dirent_info_lock(
        &self,
        inode_lock: &RwLockWriteGuard<InodeLock>,
        offset: u32,
        length: usize,
    ) -> Result<Vec<(String, usize, u64, FATDiskInodeType)>, ()>;
    
    /// Delete self directory entry
    fn delete_self_dir_ent(&self) -> Result<(), ()>;
    
    /// Unlink with lock
    fn unlink_lock(
        &self,
        _inode_lock: &RwLockWriteGuard<InodeLock>,
        delete: bool,
    ) -> Result<(), isize>;
    
    /// Get file statistics with lock
    fn stat_lock(&self, _inode_lock: &RwLockReadGuard<InodeLock>) -> (i64, i64, i64, i64, u64);
    
    /// Get inode time
    fn time(&self) -> MutexGuard<InodeTime>;
    
    /// Out-of-memory handler
    fn oom(&self) -> usize;
    
    /// Modify size with lock
    fn modify_size_lock(&self, inode_lock: &RwLockWriteGuard<InodeLock>, diff: isize, clear: bool);
    
    /// Check if directory is empty with lock
    fn is_empty_dir_lock(&self, inode_lock: &RwLockWriteGuard<InodeLock>) -> bool;

    // 从现有的目录项创建新的文件
    fn from_ent(
        &self,
        parent_dir: &Arc<dyn InodeTrait>,
        ent: &FATShortDirEnt,
        offset: u32,
    ) -> Arc<dyn InodeTrait>;
    // where
    //     Self: Sized;
    fn link_par_lock(
        &self,
        inode_lock: &RwLockWriteGuard<InodeLock>,
        parent_dir: &Arc<dyn InodeTrait>,
        parent_inode_lock: &RwLockWriteGuard<InodeLock>,
        name: String,
    ) -> Result<(), ()>;
    fn create_lock(
        &self,
        parent_dir: &Arc<dyn InodeTrait>,
        parent_inode_lock: &RwLockWriteGuard<InodeLock>,
        name: String,
        file_type: DiskInodeType,
    ) -> Result<Arc<dyn InodeTrait>, ()>;
    // where
    // Self: Sized;
    fn gen_short_name_slice(
        parent_dir: &Arc<Self>,
        parent_inode_lock: &RwLockWriteGuard<InodeLock>,
        name: &String,
    ) -> [u8; 11]
    where
        Self: Sized;
    fn gen_name_slice(
        parent_dir: &Arc<Self>,
        parent_inode_lock: &RwLockWriteGuard<InodeLock>,
        name: &String,
    ) -> ([u8; 11], Vec<[u16; 13]>)
    where
        Self: Sized;
    fn gen_long_name_slice(name: &String, long_ent_index: usize) -> [u16; 13]
    where
        Self: Sized;
    fn as_any(&self) -> &dyn Any;
    fn root_inode(efs: &Arc<dyn VFS>) -> Arc<Self>
    where
        Self: Sized;
}
impl_downcast!(sync InodeTrait);

pub struct InodeTime {
    create_time: u64,
    access_time: u64,
    modify_time: u64,
}
#[allow(unused)]
impl InodeTime {
    pub fn new() -> Self {
        Self {
            create_time: 0,
            access_time: 0,
            modify_time: 0,
        }
    }
    /// 设置inode的创建时间
    pub fn set_create_time(&mut self, create_time: u64) {
        self.create_time = create_time;
    }

    /// 获取inode的创建时间的引用
    pub fn create_time(&self) -> &u64 {
        &self.create_time
    }

    /// 设置inode的访问时间
    pub fn set_access_time(&mut self, access_time: u64) {
        self.access_time = access_time;
    }

    /// 获取inode的访问时间的引用
    pub fn access_time(&self) -> &u64 {
        &self.access_time
    }

    /// 设置inode的修改时间
    pub fn set_modify_time(&mut self, modify_time: u64) {
        self.modify_time = modify_time;
    }

    /// 获取inode的修改时间的引用
    pub fn modify_time(&self) -> &u64 {
        &self.modify_time
    }
}

// 文件或者目录
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum DiskInodeType {
    File,
    Directory,
    FIFO,
    Character,
    Block,
    Socket,
    Link,
}
