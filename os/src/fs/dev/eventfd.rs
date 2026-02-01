//! EventFd implementation
//!
//! Provides event notification mechanism via file descriptor

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
use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;
use core::convert::TryInto;

/// EventFd flags
pub const EFD_SEMAPHORE: u32 = 1;
pub const EFD_CLOEXEC: u32 = 0o2000000;
pub const EFD_NONBLOCK: u32 = 0o4000;

/// EventFd file descriptor
pub struct EventFd {
    /// Inner state protected by mutex
    inner: Arc<Mutex<EventFdInner>>,
    /// Flags
    flags: u32,
}

struct EventFdInner {
    /// 64-bit counter
    counter: u64,
}

impl EventFd {
    /// Create a new EventFd with initial value and flags
    pub fn new(initval: u32, flags: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(EventFdInner {
                counter: initval as u64,
            })),
            flags,
        }
    }
    
    fn is_semaphore(&self) -> bool {
        self.flags & EFD_SEMAPHORE != 0
    }
    
    fn is_nonblock(&self) -> bool {
        self.flags & EFD_NONBLOCK != 0
    }
}

impl File for EventFd {
    fn deep_clone(&self) -> Arc<dyn File> {
        Arc::new(Self {
            inner: self.inner.clone(),
            flags: self.flags,
        })
    }

    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        true
    }

    fn read(&self, offset: Option<&mut usize>, buf: &mut [u8]) -> usize {
        if offset.is_some() {
            return ESPIPE as usize;
        }
        
        // EventFd read must be at least 8 bytes
        if buf.len() < 8 {
            return EINVAL as usize;
        }
        
        loop {
            {
                let mut inner = self.inner.lock();
                
                if inner.counter > 0 {
                    // Read the value
                    let value = if self.is_semaphore() {
                        // Semaphore mode: return 1 and decrement
                        inner.counter -= 1;
                        1u64
                    } else {
                        // Normal mode: return counter and reset to 0
                        let v = inner.counter;
                        inner.counter = 0;
                        v
                    };
                    
                    // Write value to buffer
                    buf[..8].copy_from_slice(&value.to_ne_bytes());
                    return 8;
                }
                
                // Counter is 0
                if self.is_nonblock() {
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

    fn write(&self, offset: Option<&mut usize>, buf: &[u8]) -> usize {
        if offset.is_some() {
            return ESPIPE as usize;
        }
        
        // EventFd write must be exactly 8 bytes
        if buf.len() < 8 {
            return EINVAL as usize;
        }
        
        let value = u64::from_ne_bytes(buf[..8].try_into().unwrap());
        
        // Value 0xFFFFFFFFFFFFFFFF is invalid
        if value == u64::MAX {
            return EINVAL as usize;
        }
        
        loop {
            {
                let mut inner = self.inner.lock();
                
                // Check for overflow
                if inner.counter <= u64::MAX - value - 1 {
                    inner.counter += value;
                    return 8;
                }
                
                // Would overflow
                if self.is_nonblock() {
                    return EAGAIN as usize;
                }
            }
            
            // Block and wait for space
            let task = current_task().unwrap();
            wait_with_timeout(Arc::downgrade(&task), TimeSpec::now());
            drop(task);
            block_current_and_run_next();
        }
    }

    fn r_ready(&self) -> bool {
        self.inner.lock().counter > 0
    }

    fn w_ready(&self) -> bool {
        self.inner.lock().counter < u64::MAX - 1
    }

    fn read_user(&self, offset: Option<usize>, mut buf: UserBuffer) -> usize {
        let mut tmp = [0u8; 8];
        let offset_mut = offset.map(|mut o| &mut o as *mut usize);
        let result = self.read(
            unsafe { offset_mut.map(|p| &mut *p) },
            &mut tmp,
        );
        if result == 8 {
            buf.write(&tmp);
        }
        result
    }

    fn write_user(&self, offset: Option<usize>, buf: UserBuffer) -> usize {
        let mut tmp = [0u8; 8];
        buf.read(&mut tmp);
        let offset_mut = offset.map(|mut o| &mut o as *mut usize);
        self.write(
            unsafe { offset_mut.map(|p| &mut *p) },
            &tmp,
        )
    }

    fn get_size(&self) -> usize {
        0
    }

    fn get_stat(&self) -> Stat {
        Stat::new(
            crate::makedev!(0, 1),  // st_dev
            1,                       // st_ino
            StatMode::S_IFCHR.bits() | 0o666,  // st_mode
            1,                       // st_nlink
            0,                       // st_rdev
            0,                       // st_size
            0,                       // st_atime_sec
            0,                       // st_mtime_sec
            0,                       // st_ctime_sec
        )
    }

    fn get_file_type(&self) -> DiskInodeType {
        DiskInodeType::File
    }

    fn info_dirtree_node(&self, _dirnode_ptr: Weak<DirectoryTreeNode>) {}

    fn get_dirtree_node(&self) -> Option<Arc<DirectoryTreeNode>> {
        None
    }

    fn open(&self, _flags: crate::fs::OpenFlags, _special_use: bool) -> Arc<dyn File> {
        self.deep_clone()
    }

    fn open_subfile(&self) -> Result<Vec<(String, Arc<dyn File>)>, isize> {
        Err(ENOTDIR)
    }

    fn create(&self, _name: &str, _file_type: DiskInodeType) -> Result<Arc<dyn File>, isize> {
        Err(ENOTDIR)
    }

    fn link_child(&self, _name: &str, _child: &Self) -> Result<(), isize>
    where
        Self: Sized,
    {
        Err(EPERM)
    }

    fn unlink(&self, _delete: bool) -> Result<(), isize> {
        Err(EPERM)
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

    fn set_timestamp(
        &self,
        _ctime: Option<usize>,
        _atime: Option<usize>,
        _mtime: Option<usize>,
    ) {
    }

    fn get_single_cache(
        &self,
        _offset: usize,
    ) -> Result<Arc<Mutex<crate::fs::cache::PageCache>>, ()> {
        Err(())
    }

    fn get_all_caches(&self) -> Result<Vec<Arc<Mutex<crate::fs::cache::PageCache>>>, ()> {
        Err(())
    }

    fn ioctl(&self, _cmd: u32, _argp: usize) -> isize {
        ENOTTY
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
