//! EventFd implementation
//!
//! EventFd provides a lightweight inter-process communication (IPC) mechanism
//! using a 64-bit counter. It supports both semaphore and non-semaphore modes.

use crate::fs::directory_tree::DirectoryTreeNode;
use crate::fs::dirent::Dirent;
use crate::fs::layout::Stat;
use crate::fs::DiskInodeType;
use crate::fs::StatMode;
use crate::syscall::errno::*;
use crate::task::block_current_and_run_next;
use crate::task::current_task;
use crate::task::wait_with_timeout;
use crate::timer::TimeSpec;
use crate::fs::file_trait::File;
use crate::mm::UserBuffer;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::Mutex;
use core::mem::size_of;
use log::info;

/// EventFd flags
pub const EFD_SEMAPHORE: u32 = 1;
pub const EFD_CLOEXEC: u32 = 0o2000000;
pub const EFD_NONBLOCK: u32 = 0o4000;

/// EventFd file type
pub struct EventFd {
    /// The 64-bit counter value
    counter: Mutex<u64>,
    /// Whether this eventfd is in semaphore mode
    semaphore: bool,
    /// Whether reads should be non-blocking
    nonblock: bool,
}

impl EventFd {
    /// Create a new EventFd with the given initial value and flags
    pub fn new(initval: u32, flags: u32) -> Self {
        Self {
            counter: Mutex::new(initval as u64),
            semaphore: (flags & EFD_SEMAPHORE) != 0,
            nonblock: (flags & EFD_NONBLOCK) != 0,
        }
    }
    
    /// Check if the counter is readable (non-zero)
    fn is_readable(&self) -> bool {
        *self.counter.lock() > 0
    }
    
    /// Check if the counter can be written (won't overflow)
    fn is_writable(&self) -> bool {
        *self.counter.lock() < u64::MAX - 1
    }
}

impl File for EventFd {
    fn deep_clone(&self) -> Arc<dyn File> {
        let counter_val = *self.counter.lock();
        Arc::new(EventFd {
            counter: Mutex::new(counter_val),
            semaphore: self.semaphore,
            nonblock: self.nonblock,
        })
    }

    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        true
    }

    /// Read from eventfd
    /// 
    /// Each read returns an 8-byte integer. If EFD_SEMAPHORE is set,
    /// returns 1 and decrements counter by 1. Otherwise returns counter
    /// value and resets counter to 0.
    fn read(&self, offset: Option<&mut usize>, buf: &mut [u8]) -> usize {
        if offset.is_some() {
            return ESPIPE as usize;
        }
        
        // Buffer must be at least 8 bytes
        if buf.len() < size_of::<u64>() {
            return EINVAL as usize;
        }
        
        loop {
            {
                let mut counter = self.counter.lock();
                if *counter > 0 {
                    let value = if self.semaphore {
                        *counter -= 1;
                        1u64
                    } else {
                        let val = *counter;
                        *counter = 0;
                        val
                    };
                    
                    // Write the value to the buffer (little-endian)
                    buf[..8].copy_from_slice(&value.to_ne_bytes());
                    return size_of::<u64>();
                }
                
                // Counter is 0
                if self.nonblock {
                    return EAGAIN as usize;
                }
            }
            
            // Block and wait
            let task = current_task().unwrap();
            wait_with_timeout(Arc::downgrade(&task), TimeSpec::now());
            drop(task);
            block_current_and_run_next();
        }
    }

    /// Write to eventfd
    /// 
    /// Adds the 8-byte integer value to the counter. If adding would
    /// cause overflow past u64::MAX - 1, either blocks or returns EAGAIN.
    fn write(&self, offset: Option<&mut usize>, buf: &[u8]) -> usize {
        if offset.is_some() {
            return ESPIPE as usize;
        }
        
        // Buffer must be at least 8 bytes
        if buf.len() < size_of::<u64>() {
            return EINVAL as usize;
        }
        
        // Read the value from buffer (little-endian)
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&buf[..8]);
        let value = u64::from_ne_bytes(value_bytes);
        
        // Value of 0xFFFFFFFFFFFFFFFF is invalid
        if value == u64::MAX {
            return EINVAL as usize;
        }
        
        loop {
            {
                let mut counter = self.counter.lock();
                // Check if we can add without overflow
                if *counter <= u64::MAX - 1 - value {
                    *counter += value;
                    return size_of::<u64>();
                }
                
                // Would overflow
                if self.nonblock {
                    return EAGAIN as usize;
                }
            }
            
            // Block and wait
            let task = current_task().unwrap();
            wait_with_timeout(Arc::downgrade(&task), TimeSpec::now());
            drop(task);
            block_current_and_run_next();
        }
    }

    fn r_ready(&self) -> bool {
        self.is_readable()
    }

    fn w_ready(&self) -> bool {
        self.is_writable()
    }

    fn read_user(&self, offset: Option<usize>, mut buf: UserBuffer) -> usize {
        if offset.is_some() {
            return ESPIPE as usize;
        }
        
        // Need at least 8 bytes
        let total_len: usize = buf.buffers.iter().map(|b| b.len()).sum();
        if total_len < size_of::<u64>() {
            return EINVAL as usize;
        }
        
        loop {
            let mut counter = self.counter.lock();
            if *counter > 0 {
                let value = if self.semaphore {
                    *counter -= 1;
                    1u64
                } else {
                    let val = *counter;
                    *counter = 0;
                    val
                };
                drop(counter);
                
                // Write to user buffer
                let value_bytes = value.to_ne_bytes();
                let mut written = 0;
                for buffer in buf.buffers.iter_mut() {
                    let to_write = (8 - written).min(buffer.len());
                    if to_write > 0 {
                        buffer[..to_write].copy_from_slice(&value_bytes[written..written + to_write]);
                        written += to_write;
                    }
                    if written >= 8 {
                        break;
                    }
                }
                return size_of::<u64>();
            }
            
            if self.nonblock {
                return EAGAIN as usize;
            }
            
            drop(counter);
            
            let task = current_task().unwrap();
            wait_with_timeout(Arc::downgrade(&task), TimeSpec::now());
            drop(task);
            block_current_and_run_next();
        }
    }

    fn write_user(&self, offset: Option<usize>, buf: UserBuffer) -> usize {
        if offset.is_some() {
            return ESPIPE as usize;
        }
        
        // Need at least 8 bytes
        let total_len: usize = buf.buffers.iter().map(|b| b.len()).sum();
        if total_len < size_of::<u64>() {
            return EINVAL as usize;
        }
        
        // Read value from user buffer
        let mut value_bytes = [0u8; 8];
        let mut read = 0;
        for buffer in buf.buffers.iter() {
            let to_read = (8 - read).min(buffer.len());
            if to_read > 0 {
                value_bytes[read..read + to_read].copy_from_slice(&buffer[..to_read]);
                read += to_read;
            }
            if read >= 8 {
                break;
            }
        }
        let value = u64::from_ne_bytes(value_bytes);
        
        if value == u64::MAX {
            return EINVAL as usize;
        }
        
        loop {
            {
                let mut counter = self.counter.lock();
                if *counter <= u64::MAX - 1 - value {
                    *counter += value;
                    return size_of::<u64>();
                }
                
                if self.nonblock {
                    return EAGAIN as usize;
                }
            }
            
            let task = current_task().unwrap();
            wait_with_timeout(Arc::downgrade(&task), TimeSpec::now());
            drop(task);
            block_current_and_run_next();
        }
    }

    fn get_size(&self) -> usize {
        0
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            crate::makedev!(0, 0),
            1,
            StatMode::S_IFREG.bits() | 0o666,
            1,
            0,
            0,
            0,
            0,
            0,
        )
    }

    fn get_file_type(&self) -> DiskInodeType {
        DiskInodeType::File
    }

    fn info_dirtree_node(&self, _dirnode_ptr: Weak<DirectoryTreeNode>) {
        // EventFd is not associated with directory tree
    }

    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> {
        None
    }

    fn open(&self, _flags: crate::fs::layout::OpenFlags, _special_use: bool) -> Arc<dyn File> {
        self.deep_clone()
    }

    fn open_subfile(&self) -> Result<Vec<(alloc::string::String, Arc<dyn File>)>, isize> {
        Err(ENOTDIR)
    }

    fn create(&self, _name: &str, _file_type: DiskInodeType) -> Result<Arc<dyn File>, isize> {
        Err(ENOTDIR)
    }

    fn link_child(&self, _name: &str, _child: &Self) -> Result<(), isize>
    where
        Self: Sized,
    {
        Err(ENOTDIR)
    }

    fn unlink(&self, _delete: bool) -> Result<(), isize> {
        Err(ENOENT)
    }

    fn get_dirent(&self, _count: usize) -> Vec<Dirent> {
        Vec::new()
    }

    fn lseek(&self, _offset: isize, _whence: crate::fs::SeekWhence) -> Result<usize, isize> {
        Err(ESPIPE)
    }

    fn modify_size(&self, _diff: isize) -> Result<(), isize> {
        Err(EINVAL)
    }

    fn truncate_size(&self, _new_size: usize) -> Result<(), isize> {
        Err(EINVAL)
    }

    fn set_timestamp(&self, _ctime: Option<usize>, _atime: Option<usize>, _mtime: Option<usize>) {
        // EventFd doesn't support timestamps
    }

    fn get_single_cache(&self, _offset: usize) -> Result<Arc<spin::Mutex<crate::fs::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(&self) -> Result<Vec<Arc<spin::Mutex<crate::fs::PageCache>>>, ()> {
        Err(())
    }

    fn oom(&self) -> usize {
        0
    }

    fn hang_up(&self) -> bool {
        false
    }

    fn fcntl(&self, _cmd: u32, _arg: u32) -> isize {
        SUCCESS
    }
}
