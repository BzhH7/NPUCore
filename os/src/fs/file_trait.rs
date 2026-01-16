//! File trait definition
//!
//! Defines the common interface for all file-like objects including:
//! - Regular files
//! - Directories
//! - Device files
//! - Pipes and sockets

use super::{dirent::Dirent, fat32::DiskInodeType};
use crate::{mm::UserBuffer, syscall::errno::ENOTTY};
use __alloc::string::String;
use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use downcast_rs::*;
use spin::Mutex;

use super::{cache::PageCache, directory_tree::DirectoryTreeNode, layout::*};

/// Common file interface
///
/// All file-like objects (files, directories, devices, pipes) implement this trait
pub trait File: DowncastSync {
    /// Create a deep clone of the file descriptor
    fn deep_clone(&self) -> Arc<dyn File>;
    
    /// Check if file is readable
    fn readable(&self) -> bool;
    
    /// Check if file is writable
    fn writable(&self) -> bool;
    
    /// Read from file
    ///
    /// # Arguments
    /// * `offset` - Optional read offset (updates if provided)
    /// * `buf` - Buffer to read into
    fn read(&self, offset: Option<&mut usize>, buf: &mut [u8]) -> usize;
    
    /// Write to file
    ///
    /// # Arguments
    /// * `offset` - Optional write offset (updates if provided)
    /// * `buf` - Data to write
    fn write(&self, offset: Option<&mut usize>, buf: &[u8]) -> usize;
    
    /// Check if file is ready for reading
    fn r_ready(&self) -> bool;
    
    /// Check if file is ready for writing
    fn w_ready(&self) -> bool;
    
    /// Read from file into user buffer
    fn read_user(&self, offset: Option<usize>, buf: UserBuffer) -> usize;
    
    /// Write from user buffer to file
    fn write_user(&self, offset: Option<usize>, buf: UserBuffer) -> usize;
    
    /// Get file size
    fn get_size(&self) -> usize;
    
    /// Get file statistics
    fn get_stat(&self) -> Stat;
    
    /// Get file type
    /// Get file type
    fn get_file_type(&self) -> DiskInodeType;
    
    /// Check if file is a directory
    fn is_dir(&self) -> bool {
        self.get_file_type() == DiskInodeType::Directory
    }
    
    /// Check if file is a regular file
    fn is_file(&self) -> bool {
        self.get_file_type() == DiskInodeType::File
    }
    
    /// Associate directory tree node with this file
    fn info_dirtree_node(&self, dirnode_ptr: Weak<DirectoryTreeNode>);
    
    /// Get associated directory tree node
    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>>;
    
    /// Open file with flags
    ///
    /// # Arguments
    /// * `flags` - Open flags (read/write/create/etc.)
    /// * `special_use` - Special usage flag
    fn open(&self, flags: OpenFlags, special_use: bool) -> Arc<dyn File>;
    
    /// Open subfiles (for directories)
    fn open_subfile(&self) -> Result<Vec<(String, Arc<dyn File>)>, isize>;
    
    /// Create a new file or directory
    ///
    /// # Arguments
    /// * `name` - File name
    /// * `file_type` - Type of file to create
    fn create(&self, name: &str, file_type: DiskInodeType) -> Result<Arc<dyn File>, isize>;
    
    /// Link a child file
    fn link_child(&self, name: &str, child: &Self) -> Result<(), isize>
    where
        Self: Sized;
    
    /// Unlink/delete file
    ///
    /// # Arguments
    /// * `delete` - Whether to actually delete or just unlink
    fn unlink(&self, delete: bool) -> Result<(), isize>;
    
    /// Get directory entries
    ///
    /// # Arguments
    /// * `count` - Maximum number of entries to return
    fn get_dirent(&self, count: usize) -> Vec<Dirent>;
    
    /// Get current file offset
    fn get_offset(&self) -> usize {
        self.lseek(0, SeekWhence::SEEK_CUR).unwrap()
    }
    
    /// Seek to position
    ///
    /// # Arguments
    /// * `offset` - Offset value
    /// * `whence` - Seek origin (SET/CUR/END)
    fn lseek(&self, offset: isize, whence: SeekWhence) -> Result<usize, isize>;
    /// size
    fn modify_size(&self, diff: isize) -> Result<(), isize>;
    fn truncate_size(&self, new_size: usize) -> Result<(), isize>;
    // time
    fn set_timestamp(&self, ctime: Option<usize>, atime: Option<usize>, mtime: Option<usize>);
    /// cache
    fn get_single_cache(&self, offset: usize) -> Result<Arc<Mutex<PageCache>>, ()>;
    fn get_all_caches(&self) -> Result<Vec<Arc<Mutex<PageCache>>>, ()>;
    /// memory related
    fn oom(&self) -> usize;
    /// poll, select related
    fn hang_up(&self) -> bool;
    /// iotcl
    fn ioctl(&self, _cmd: u32, _argp: usize) -> isize {
        ENOTTY
    }
    /// fcntl
    fn fcntl(&self, cmd: u32, arg: u32) -> isize;
}
impl_downcast!(sync File);
