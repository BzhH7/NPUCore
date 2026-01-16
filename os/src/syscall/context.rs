//! Syscall execution context abstraction
//!
//! This module provides a unified context for syscall execution that encapsulates
//! task state access patterns. Instead of repeatedly calling `current_task().unwrap()`
//! and acquiring various locks throughout syscall implementations, we use a context
//! object that provides safe, structured access to task resources.
//!
//! # Architecture
//!
//! The `SyscallContext` struct serves as the primary interface for syscalls to access:
//! - Current task's file descriptor table
//! - Virtual memory operations
//! - Socket table
//! - Filesystem state
//! - Signal handling
//!
//! # Usage Example
//!
//! ```rust
//! pub fn sys_read(fd: usize, buf: usize, len: usize) -> isize {
//!     SyscallContext::execute(|ctx| {
//!         let file = ctx.lookup_fd(fd)?;
//!         // ... perform read operation
//!         Ok(bytes_read)
//!     })
//! }
//! ```

use alloc::sync::Arc;
use core::ops::Deref;
use spin::MutexGuard;

use crate::fs::file_descriptor::FdTable;
use crate::fs::FileDescriptor;
use crate::mm::{MemorySet, MapPermission, VirtAddr};
use crate::mm::PageTableImpl;
use crate::net::SocketTable;
use crate::task::{current_task, current_user_token, TaskControlBlock};
use crate::task::task::FsStatus;

use super::errno::*;

/// Error types specific to syscall context operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextError {
    /// No current task available (kernel bug or early boot)
    NoCurrentTask,
    /// Invalid file descriptor
    InvalidFd(usize),
    /// Memory access violation
    BadAddress(usize),
    /// Permission denied for operation
    PermissionDenied,
    /// Resource temporarily unavailable
    WouldBlock,
    /// Invalid argument provided
    InvalidArgument,
}

impl ContextError {
    /// Convert context error to errno for syscall return
    #[inline]
    pub const fn as_errno(&self) -> isize {
        match self {
            Self::NoCurrentTask => ESRCH,
            Self::InvalidFd(_) => EBADF,
            Self::BadAddress(_) => EFAULT,
            Self::PermissionDenied => EACCES,
            Self::WouldBlock => EAGAIN,
            Self::InvalidArgument => EINVAL,
        }
    }
}

impl From<ContextError> for isize {
    #[inline]
    fn from(err: ContextError) -> isize {
        err.as_errno()
    }
}

/// Result type for syscall context operations
pub type ContextResult<T> = Result<T, ContextError>;

/// Syscall result that can be converted to isize return value
pub type SyscallResult = Result<usize, isize>;

/// Syscall execution context
///
/// Provides structured access to task resources during syscall execution.
/// This abstraction reduces code duplication and ensures consistent error handling
/// across all syscall implementations.
pub struct SyscallContext {
    task: Arc<TaskControlBlock>,
    user_token: usize,
}

impl SyscallContext {
    /// Create a new syscall context for the current task
    ///
    /// # Returns
    /// - `Ok(SyscallContext)` if current task exists
    /// - `Err(ContextError::NoCurrentTask)` if no task is running
    #[inline]
    pub fn acquire() -> ContextResult<Self> {
        let task = current_task().ok_or(ContextError::NoCurrentTask)?;
        let user_token = current_user_token();
        Ok(Self { task, user_token })
    }

    /// Execute a syscall with automatic context acquisition
    ///
    /// This is the primary entry point for syscall implementations.
    /// It handles context acquisition and converts the result to isize.
    ///
    /// # Arguments
    /// * `f` - Closure that receives the context and returns SyscallResult
    ///
    /// # Returns
    /// * Positive/zero value on success
    /// * Negative errno on failure
    #[inline]
    pub fn execute<F>(f: F) -> isize
    where
        F: FnOnce(&Self) -> SyscallResult,
    {
        match Self::acquire() {
            Ok(ctx) => match f(&ctx) {
                Ok(val) => val as isize,
                Err(errno) => errno,
            },
            Err(e) => e.as_errno(),
        }
    }

    /// Execute with mutable context access
    #[inline]
    pub fn execute_mut<F>(f: F) -> isize
    where
        F: FnOnce(&mut Self) -> SyscallResult,
    {
        match Self::acquire() {
            Ok(mut ctx) => match f(&mut ctx) {
                Ok(val) => val as isize,
                Err(errno) => errno,
            },
            Err(e) => e.as_errno(),
        }
    }

    /// Get reference to the underlying task control block
    #[inline]
    pub fn task(&self) -> &Arc<TaskControlBlock> {
        &self.task
    }

    /// Get user space page table token
    #[inline]
    pub fn user_token(&self) -> usize {
        self.user_token
    }

    /// Get process ID (tgid)
    #[inline]
    pub fn pid(&self) -> usize {
        self.task.tgid
    }

    /// Get thread ID
    #[inline]
    pub fn tid(&self) -> usize {
        self.task.tid
    }

    /// Acquire file descriptor table lock
    #[inline]
    pub fn fd_table(&self) -> MutexGuard<'_, FdTable> {
        self.task.files.lock()
    }

    /// Acquire virtual memory lock
    #[inline]
    pub fn memory(&self) -> MutexGuard<'_, MemorySet<PageTableImpl>> {
        self.task.vm.lock()
    }

    /// Acquire socket table lock
    #[inline]
    pub fn sockets(&self) -> MutexGuard<'_, SocketTable> {
        self.task.socket_table.lock()
    }

    /// Acquire filesystem state lock
    #[inline]
    pub fn fs_state(&self) -> MutexGuard<'_, FsStatus> {
        self.task.fs.lock()
    }

    /// Look up a file descriptor by number
    ///
    /// # Arguments
    /// * `fd` - File descriptor number
    ///
    /// # Returns
    /// * `Ok(&FileDescriptor)` if fd is valid
    /// * `Err(EBADF)` if fd is invalid
    #[inline]
    pub fn lookup_fd(&self, fd: usize) -> Result<FileDescriptor, isize> {
        let fd_table = self.fd_table();
        match fd_table.get_ref(fd) {
            Ok(file_desc) => Ok(file_desc.clone()),
            Err(errno) => Err(errno),
        }
    }

    /// Check if a file descriptor is valid
    #[inline]
    pub fn fd_exists(&self, fd: usize) -> bool {
        self.fd_table().get_ref(fd).is_ok()
    }

    /// Validate a user buffer for the given permissions
    ///
    /// # Arguments
    /// * `addr` - Start address of buffer
    /// * `len` - Length of buffer
    /// * `perm` - Required permissions (R/W/X)
    ///
    /// # Returns
    /// * `Ok(())` if buffer is valid
    /// * `Err(EFAULT)` if buffer is invalid
    #[inline]
    pub fn validate_buffer(&self, addr: usize, len: usize, perm: MapPermission) -> Result<(), isize> {
        if self.memory().contains_valid_buffer(addr, len, perm) {
            Ok(())
        } else {
            Err(EFAULT)
        }
    }

    /// Validate a user buffer for reading
    #[inline]
    pub fn validate_read_buffer(&self, addr: usize, len: usize) -> Result<(), isize> {
        self.validate_buffer(addr, len, MapPermission::R)
    }

    /// Validate a user buffer for writing
    #[inline]
    pub fn validate_write_buffer(&self, addr: usize, len: usize) -> Result<(), isize> {
        self.validate_buffer(addr, len, MapPermission::W)
    }

    /// Get the current working directory file descriptor
    #[inline]
    pub fn working_dir(&self) -> Arc<FileDescriptor> {
        Arc::clone(&self.fs_state().working_inode)
    }

    /// Acquire inner task lock for signal/state operations
    #[inline]
    pub fn inner_lock(&self) -> spin::MutexGuard<'_, crate::task::task::TaskControlBlockInner> {
        self.task.acquire_inner_lock()
    }
}

/// Trait for converting syscall arguments
pub trait FromSyscallArg: Sized {
    /// Convert from raw syscall argument
    fn from_arg(arg: usize) -> Self;
}

impl FromSyscallArg for usize {
    #[inline]
    fn from_arg(arg: usize) -> Self {
        arg
    }
}

impl FromSyscallArg for isize {
    #[inline]
    fn from_arg(arg: usize) -> Self {
        arg as isize
    }
}

impl FromSyscallArg for u32 {
    #[inline]
    fn from_arg(arg: usize) -> Self {
        arg as u32
    }
}

impl FromSyscallArg for i32 {
    #[inline]
    fn from_arg(arg: usize) -> Self {
        arg as i32
    }
}

impl<T> FromSyscallArg for *const T {
    #[inline]
    fn from_arg(arg: usize) -> Self {
        arg as *const T
    }
}

impl<T> FromSyscallArg for *mut T {
    #[inline]
    fn from_arg(arg: usize) -> Self {
        arg as *mut T
    }
}

/// Helper macro to simplify syscall implementation with context
#[macro_export]
macro_rules! with_context {
    ($body:expr) => {
        $crate::syscall::context::SyscallContext::execute(|ctx| $body(ctx))
    };
}

/// Helper macro for syscalls that need mutable context
#[macro_export]
macro_rules! with_context_mut {
    ($body:expr) => {
        $crate::syscall::context::SyscallContext::execute_mut(|ctx| $body(ctx))
    };
}

// ============================================================================
// File Descriptor Operation Helpers
// ============================================================================

/// Execute an operation with a file descriptor
///
/// This helper acquires the fd table, looks up the descriptor, and executes
/// the provided closure with the file reference. Reduces boilerplate in syscalls.
///
/// # Arguments
/// * `fd` - File descriptor number
/// * `operation` - Closure receiving (task, file_descriptor) that returns SyscallResult
///
/// # Example
/// ```rust
/// with_fd(fd, |task, file| {
///     let bytes = file.read(buffer)?;
///     Ok(bytes)
/// })
/// ```
#[inline]
pub fn with_fd<F>(fd: usize, operation: F) -> isize 
where
    F: FnOnce(&Arc<TaskControlBlock>, &FileDescriptor) -> Result<usize, isize>
{
    let Some(task) = current_task() else {
        return ESRCH;
    };
    
    let fd_table = task.files.lock();
    let file = match fd_table.get_ref(fd) {
        Ok(f) => f,
        Err(errno) => return errno,
    };
    
    match operation(&task, file) {
        Ok(result) => result as isize,
        Err(errno) => errno,
    }
}

/// Execute an operation with two file descriptors
///
/// Useful for operations like dup2, sendfile, splice that need two fds.
#[inline]
pub fn with_two_fds<F>(fd1: usize, fd2: usize, operation: F) -> isize
where
    F: FnOnce(&Arc<TaskControlBlock>, &FileDescriptor, &FileDescriptor) -> Result<usize, isize>
{
    let Some(task) = current_task() else {
        return ESRCH;
    };
    
    let fd_table = task.files.lock();
    
    let file1 = match fd_table.get_ref(fd1) {
        Ok(f) => f,
        Err(errno) => return errno,
    };
    
    let file2 = match fd_table.get_ref(fd2) {
        Ok(f) => f,
        Err(errno) => return errno,
    };
    
    match operation(&task, file1, file2) {
        Ok(result) => result as isize,
        Err(errno) => errno,
    }
}

/// Execute an operation with mutable fd_table access
///
/// For operations that need to modify the fd table (open, close, dup).
#[inline]
pub fn with_fd_table_mut<F>(operation: F) -> isize
where
    F: FnOnce(&Arc<TaskControlBlock>, &mut FdTable) -> Result<usize, isize>
{
    let Some(task) = current_task() else {
        return ESRCH;
    };
    
    let mut fd_table = task.files.lock();
    
    match operation(&task, &mut fd_table) {
        Ok(result) => result as isize,
        Err(errno) => errno,
    }
}

/// Validate that fd is readable before operation
#[inline]
pub fn require_readable(file: &FileDescriptor) -> Result<(), isize> {
    if file.readable() {
        Ok(())
    } else {
        Err(EBADF)
    }
}

/// Validate that fd is writable before operation
#[inline]
pub fn require_writable(file: &FileDescriptor) -> Result<(), isize> {
    if file.writable() {
        Ok(())
    } else {
        Err(EBADF)
    }
}
