//! I/O Operation Helpers for Syscalls
//!
//! This module provides unified abstractions for file I/O operations,
//! reducing code duplication in syscall implementations. The core pattern
//! is to use generic operations with type parameters for direction.
//!
//! # Design Philosophy
//!
//! Instead of having separate implementations for read/write, pread/pwrite,
//! readv/writev, etc., we use a unified approach:
//!
//! 1. **Direction trait**: Determines if operation is read or write
//! 2. **Offset handling**: Optional offset for positioned I/O  
//! 3. **Buffer abstraction**: Handles single and vectored I/O uniformly
//!
//! # Examples
//!
//! ```rust
//! // Simple read using the helper
//! let result = io_operation::<Read>(fd, buf, count, None);
//!
//! // Positioned write
//! let result = io_operation::<Write>(fd, buf, count, Some(offset));
//! ```

use crate::fs::FileDescriptor;
use crate::mm::{translated_byte_buffer, UserBuffer};
use crate::task::{current_task, current_user_token};
use super::errno::*;
use alloc::vec::Vec;

/// Marker trait for I/O direction
pub trait IoDirection {
    /// Check if file supports this direction
    fn check_permission(file: &FileDescriptor) -> bool;
    
    /// Perform the actual I/O operation
    fn perform_io(file: &FileDescriptor, offset: Option<usize>, buffer: UserBuffer) -> usize;
}

/// Read direction marker
pub struct Read;

impl IoDirection for Read {
    #[inline]
    fn check_permission(file: &FileDescriptor) -> bool {
        file.readable()
    }
    
    #[inline]
    fn perform_io(file: &FileDescriptor, offset: Option<usize>, buffer: UserBuffer) -> usize {
        file.read_user(offset, buffer)
    }
}

/// Write direction marker  
pub struct Write;

impl IoDirection for Write {
    #[inline]
    fn check_permission(file: &FileDescriptor) -> bool {
        file.writable()
    }
    
    #[inline]
    fn perform_io(file: &FileDescriptor, offset: Option<usize>, buffer: UserBuffer) -> usize {
        file.write_user(offset, buffer)
    }
}

/// Generic I/O operation for read/write syscalls
///
/// This function encapsulates the common pattern used by read, write,
/// pread, pwrite, and similar syscalls.
///
/// # Type Parameters
/// * `D` - Direction marker (Read or Write)
///
/// # Arguments
/// * `fd` - File descriptor number
/// * `buf` - User buffer address
/// * `count` - Number of bytes
/// * `offset` - Optional file offset (None uses current position)
///
/// # Returns
/// * Number of bytes transferred on success
/// * Negative errno on failure
#[inline]
pub fn io_operation<D: IoDirection>(
    fd: usize,
    buf: usize,
    count: usize,
    offset: Option<usize>,
) -> isize {
    // Acquire current task
    let task = match current_task() {
        Some(t) => t,
        None => return ESRCH,
    };
    
    // Lock and get file descriptor
    let fd_table = task.files.lock();
    let file = match fd_table.get_ref(fd) {
        Ok(f) => f,
        Err(errno) => return errno,
    };
    
    // Verify permission
    if !D::check_permission(file) {
        return EBADF;
    }
    
    // Translate user buffer
    let token = task.get_user_token();
    let buffer = match translated_byte_buffer(token, buf as *const u8, count) {
        Ok(b) => UserBuffer::new(b),
        Err(errno) => return errno,
    };
    
    // Perform operation
    D::perform_io(file, offset, buffer) as isize
}

/// I/O vector structure for vectored operations
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IoVec {
    /// Starting address of buffer
    pub base: *const u8,
    /// Number of bytes in buffer
    pub len: usize,
}

/// Generic vectored I/O operation
///
/// Handles readv/writev/preadv/pwritev syscalls with a unified approach.
///
/// # Type Parameters
/// * `D` - Direction marker (Read or Write)
///
/// # Arguments
/// * `fd` - File descriptor number
/// * `iov` - Pointer to array of IoVec structures
/// * `iovcnt` - Number of buffers in the array
/// * `offset` - Optional file offset
///
/// # Returns
/// * Total bytes transferred on success
/// * Negative errno on failure
#[inline]
pub fn vectored_io<D: IoDirection>(
    fd: usize,
    iov: *const IoVec,
    iovcnt: usize,
    offset: Option<usize>,
) -> isize {
    // Validate iovcnt
    const IOV_MAX: usize = 1024;
    if iovcnt == 0 || iovcnt > IOV_MAX {
        return EINVAL;
    }
    
    let task = match current_task() {
        Some(t) => t,
        None => return ESRCH,
    };
    
    let fd_table = task.files.lock();
    let file = match fd_table.get_ref(fd) {
        Ok(f) => f,
        Err(errno) => return errno,
    };
    
    if !D::check_permission(file) {
        return EBADF;
    }
    
    let token = task.get_user_token();
    
    // Collect all buffers into a combined UserBuffer
    let mut all_buffers: Vec<&'static mut [u8]> = Vec::with_capacity(iovcnt * 2);
    
    for i in 0..iovcnt {
        let iov_entry = unsafe { &*iov.add(i) };
        if iov_entry.len == 0 {
            continue;
        }
        
        match translated_byte_buffer(token, iov_entry.base, iov_entry.len) {
            Ok(buffers) => {
                for buf in buffers {
                    all_buffers.push(buf);
                }
            }
            Err(errno) => return errno,
        }
    }
    
    if all_buffers.is_empty() {
        return 0;
    }
    
    D::perform_io(file, offset, UserBuffer::new(all_buffers)) as isize
}

/// Copy data between file descriptors
///
/// Implements sendfile/copy_file_range-like functionality.
///
/// # Arguments
/// * `fd_in` - Input file descriptor
/// * `fd_out` - Output file descriptor
/// * `offset_in` - Optional input offset
/// * `offset_out` - Optional output offset
/// * `count` - Maximum bytes to transfer
///
/// # Returns
/// * Bytes transferred on success
/// * Negative errno on failure
pub fn copy_between_fds(
    fd_in: usize,
    fd_out: usize,
    mut offset_in: Option<usize>,
    mut offset_out: Option<usize>,
    count: usize,
) -> isize {
    let task = match current_task() {
        Some(t) => t,
        None => return ESRCH,
    };
    
    let fd_table = task.files.lock();
    
    let in_file = match fd_table.get_ref(fd_in) {
        Ok(f) => f.clone(),
        Err(errno) => return errno,
    };
    
    let out_file = match fd_table.get_ref(fd_out) {
        Ok(f) => f.clone(),
        Err(errno) => return errno,
    };
    
    // Release fd_table lock before I/O
    drop(fd_table);
    
    if !in_file.readable() || !out_file.writable() {
        return EBADF;
    }
    
    // Use kernel buffer for transfer
    let transfer_size = count.min(65536); // Limit to 64KB per chunk
    let mut buffer = alloc::vec![0u8; transfer_size];
    
    // Read from input
    let bytes_read = in_file.read(offset_in.as_mut(), &mut buffer);
    if bytes_read == 0 {
        return 0;
    }
    
    // Write to output
    let bytes_written = out_file.write(offset_out.as_mut(), &buffer[..bytes_read]);
    
    bytes_written as isize
}

// ============================================================================
// Convenience wrappers using the generic functions
// ============================================================================

/// sys_read using generic I/O
#[inline]
pub fn generic_read(fd: usize, buf: usize, count: usize) -> isize {
    io_operation::<Read>(fd, buf, count, None)
}

/// sys_write using generic I/O
#[inline]
pub fn generic_write(fd: usize, buf: usize, count: usize) -> isize {
    io_operation::<Write>(fd, buf, count, None)
}

/// sys_pread using generic I/O
#[inline]
pub fn generic_pread(fd: usize, buf: usize, count: usize, offset: usize) -> isize {
    io_operation::<Read>(fd, buf, count, Some(offset))
}

/// sys_pwrite using generic I/O
#[inline]
pub fn generic_pwrite(fd: usize, buf: usize, count: usize, offset: usize) -> isize {
    io_operation::<Write>(fd, buf, count, Some(offset))
}

/// sys_readv using generic vectored I/O
#[inline]
pub fn generic_readv(fd: usize, iov: *const IoVec, iovcnt: usize) -> isize {
    vectored_io::<Read>(fd, iov, iovcnt, None)
}

/// sys_writev using generic vectored I/O
#[inline]
pub fn generic_writev(fd: usize, iov: *const IoVec, iovcnt: usize) -> isize {
    vectored_io::<Write>(fd, iov, iovcnt, None)
}
