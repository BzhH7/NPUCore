//! Kernel-wide result and error handling framework
//!
//! This module provides a comprehensive error handling system that replaces
//! ad-hoc error returns throughout the kernel. It offers:
//!
//! - Type-safe error representation with `KernelError` enum
//! - Automatic conversion to syscall errno values
//! - Rich error context for debugging
//! - Integration with the `?` operator for ergonomic error propagation
//!
//! # Design Philosophy
//!
//! Instead of returning raw negative integers for errors, kernel code should
//! use `KernelResult<T>` which provides better type safety and error context.
//! The error types automatically convert to errno values at syscall boundaries.
//!
//! # Example Usage
//!
//! ```rust
//! fn do_file_operation(fd: usize) -> KernelResult<usize> {
//!     let file = get_file(fd).ok_or(KernelError::BadFileDescriptor { fd })?;
//!     let bytes = file.read(&mut buf).map_err(|e| KernelError::IoError { cause: e })?;
//!     Ok(bytes)
//! }
//! ```

use core::fmt::{self, Debug, Display, Formatter};
use crate::syscall::errno;

/// Kernel-wide result type
pub type KernelResult<T> = Result<T, KernelError>;

/// Comprehensive kernel error enumeration
///
/// Each variant maps to a specific errno value and carries context about
/// the error condition for debugging purposes.
#[derive(Debug, Clone)]
pub enum KernelError {
    // ==================== Process/Task Errors ====================
    
    /// No current task is running (early boot or kernel bug)
    NoCurrentTask,
    
    /// Process with given PID not found
    ProcessNotFound { pid: usize },
    
    /// Task is in an invalid state for the operation
    InvalidTaskState { expected: &'static str },
    
    /// Cannot acquire required lock (deadlock prevention)
    LockContention { resource: &'static str },

    // ==================== File System Errors ====================
    
    /// Bad file descriptor number
    BadFileDescriptor { fd: usize },
    
    /// File or directory not found
    NotFound { path_hint: Option<&'static str> },
    
    /// Path component is not a directory
    NotADirectory,
    
    /// Target is a directory (when file expected)
    IsADirectory,
    
    /// File already exists
    AlreadyExists,
    
    /// Too many open files
    TooManyOpenFiles,
    
    /// Permission denied for file operation
    FilePermissionDenied,
    
    /// Attempted write to read-only filesystem
    ReadOnlyFilesystem,
    
    /// Directory not empty (for rmdir)
    DirectoryNotEmpty,
    
    /// Symbolic link loop detected
    SymlinkLoop,
    
    /// File name too long
    NameTooLong,
    
    /// I/O error during file operation
    IoError { detail: &'static str },
    
    /// Invalid seek position
    InvalidSeek,
    
    /// Broken pipe
    BrokenPipe,

    // ==================== Memory Errors ====================
    
    /// Bad user-space address
    BadAddress { addr: usize },
    
    /// Out of memory
    OutOfMemory,
    
    /// Invalid memory mapping parameters
    InvalidMapping { reason: &'static str },
    
    /// Memory region not found
    RegionNotFound { addr: usize },
    
    /// Page fault could not be handled
    PageFaultUnhandled { addr: usize },

    // ==================== Network Errors ====================
    
    /// Not a socket
    NotASocket { fd: usize },
    
    /// Socket operation on non-socket
    SocketError { detail: &'static str },
    
    /// Connection refused
    ConnectionRefused,
    
    /// Connection reset by peer
    ConnectionReset,
    
    /// Network unreachable
    NetworkUnreachable,
    
    /// Address already in use
    AddressInUse,
    
    /// Socket not connected
    NotConnected,
    
    /// Socket already connected
    AlreadyConnected,

    // ==================== General Errors ====================
    
    /// Invalid argument
    InvalidArgument { arg_name: &'static str },
    
    /// Operation not permitted
    OperationNotPermitted,
    
    /// Function not implemented
    NotImplemented { syscall: &'static str },
    
    /// Resource temporarily unavailable
    WouldBlock,
    
    /// Interrupted by signal
    Interrupted,
    
    /// Resource busy
    ResourceBusy,
    
    /// Operation timed out
    TimedOut,
    
    /// Generic error with errno
    Errno(isize),
}

impl KernelError {
    /// Convert error to negative errno value for syscall return
    pub const fn as_errno(&self) -> isize {
        match self {
            // Process errors
            Self::NoCurrentTask => errno::ESRCH,
            Self::ProcessNotFound { .. } => errno::ESRCH,
            Self::InvalidTaskState { .. } => errno::EINVAL,
            Self::LockContention { .. } => errno::EBUSY,
            
            // File system errors
            Self::BadFileDescriptor { .. } => errno::EBADF,
            Self::NotFound { .. } => errno::ENOENT,
            Self::NotADirectory => errno::ENOTDIR,
            Self::IsADirectory => errno::EISDIR,
            Self::AlreadyExists => errno::EEXIST,
            Self::TooManyOpenFiles => errno::EMFILE,
            Self::FilePermissionDenied => errno::EACCES,
            Self::ReadOnlyFilesystem => errno::EROFS,
            Self::DirectoryNotEmpty => errno::ENOTEMPTY,
            Self::SymlinkLoop => errno::ELOOP,
            Self::NameTooLong => errno::ENAMETOOLONG,
            Self::IoError { .. } => errno::EIO,
            Self::InvalidSeek => errno::ESPIPE,
            Self::BrokenPipe => errno::EPIPE,
            
            // Memory errors
            Self::BadAddress { .. } => errno::EFAULT,
            Self::OutOfMemory => errno::ENOMEM,
            Self::InvalidMapping { .. } => errno::EINVAL,
            Self::RegionNotFound { .. } => errno::ENOMEM,
            Self::PageFaultUnhandled { .. } => errno::EFAULT,
            
            // Network errors
            Self::NotASocket { .. } => errno::ENOTSOCK,
            Self::SocketError { .. } => errno::EIO,
            Self::ConnectionRefused => errno::ECONNREFUSED,
            Self::ConnectionReset => errno::ECONNRESET,
            Self::NetworkUnreachable => errno::ENETUNREACH,
            Self::AddressInUse => errno::EADDRINUSE,
            Self::NotConnected => errno::ENOTCONN,
            Self::AlreadyConnected => errno::EISCONN,
            
            // General errors
            Self::InvalidArgument { .. } => errno::EINVAL,
            Self::OperationNotPermitted => errno::EPERM,
            Self::NotImplemented { .. } => errno::ENOSYS,
            Self::WouldBlock => errno::EAGAIN,
            Self::Interrupted => errno::EINTR,
            Self::ResourceBusy => errno::EBUSY,
            Self::TimedOut => errno::ETIMEDOUT,
            Self::Errno(e) => *e,
        }
    }

    /// Create error from raw errno value
    #[inline]
    pub const fn from_errno(errno: isize) -> Self {
        Self::Errno(errno)
    }

    /// Check if this is a "would block" error (for non-blocking I/O)
    #[inline]
    pub const fn is_would_block(&self) -> bool {
        matches!(self, Self::WouldBlock)
    }

    /// Check if this is an interrupt error
    #[inline]
    pub const fn is_interrupted(&self) -> bool {
        matches!(self, Self::Interrupted)
    }
}

impl Display for KernelError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCurrentTask => write!(f, "no current task running"),
            Self::ProcessNotFound { pid } => write!(f, "process {} not found", pid),
            Self::InvalidTaskState { expected } => write!(f, "invalid task state, expected {}", expected),
            Self::LockContention { resource } => write!(f, "lock contention on {}", resource),
            
            Self::BadFileDescriptor { fd } => write!(f, "bad file descriptor {}", fd),
            Self::NotFound { path_hint } => {
                if let Some(hint) = path_hint {
                    write!(f, "not found: {}", hint)
                } else {
                    write!(f, "file or directory not found")
                }
            }
            Self::NotADirectory => write!(f, "not a directory"),
            Self::IsADirectory => write!(f, "is a directory"),
            Self::AlreadyExists => write!(f, "file already exists"),
            Self::TooManyOpenFiles => write!(f, "too many open files"),
            Self::FilePermissionDenied => write!(f, "permission denied"),
            Self::ReadOnlyFilesystem => write!(f, "read-only filesystem"),
            Self::DirectoryNotEmpty => write!(f, "directory not empty"),
            Self::SymlinkLoop => write!(f, "too many symbolic links"),
            Self::NameTooLong => write!(f, "file name too long"),
            Self::IoError { detail } => write!(f, "I/O error: {}", detail),
            Self::InvalidSeek => write!(f, "invalid seek"),
            Self::BrokenPipe => write!(f, "broken pipe"),
            
            Self::BadAddress { addr } => write!(f, "bad address: {:#x}", addr),
            Self::OutOfMemory => write!(f, "out of memory"),
            Self::InvalidMapping { reason } => write!(f, "invalid mapping: {}", reason),
            Self::RegionNotFound { addr } => write!(f, "memory region not found at {:#x}", addr),
            Self::PageFaultUnhandled { addr } => write!(f, "page fault at {:#x}", addr),
            
            Self::NotASocket { fd } => write!(f, "fd {} is not a socket", fd),
            Self::SocketError { detail } => write!(f, "socket error: {}", detail),
            Self::ConnectionRefused => write!(f, "connection refused"),
            Self::ConnectionReset => write!(f, "connection reset"),
            Self::NetworkUnreachable => write!(f, "network unreachable"),
            Self::AddressInUse => write!(f, "address already in use"),
            Self::NotConnected => write!(f, "socket not connected"),
            Self::AlreadyConnected => write!(f, "socket already connected"),
            
            Self::InvalidArgument { arg_name } => write!(f, "invalid argument: {}", arg_name),
            Self::OperationNotPermitted => write!(f, "operation not permitted"),
            Self::NotImplemented { syscall } => write!(f, "syscall {} not implemented", syscall),
            Self::WouldBlock => write!(f, "operation would block"),
            Self::Interrupted => write!(f, "interrupted by signal"),
            Self::ResourceBusy => write!(f, "resource busy"),
            Self::TimedOut => write!(f, "operation timed out"),
            Self::Errno(e) => write!(f, "errno {}", e),
        }
    }
}

impl From<KernelError> for isize {
    #[inline]
    fn from(err: KernelError) -> isize {
        err.as_errno()
    }
}

impl From<isize> for KernelError {
    #[inline]
    fn from(errno: isize) -> Self {
        Self::Errno(errno)
    }
}

// Convenience type aliases for common patterns
pub type FileResult<T> = Result<T, KernelError>;
pub type MemResult<T> = Result<T, KernelError>;
pub type TaskResult<T> = Result<T, KernelError>;
pub type NetResult<T> = Result<T, KernelError>;

/// Extension trait for Option to convert to KernelError
pub trait OptionExt<T> {
    /// Convert None to the specified error
    fn ok_or_kernel_err(self, err: KernelError) -> KernelResult<T>;
    
    /// Convert None to BadFileDescriptor error
    fn ok_or_bad_fd(self, fd: usize) -> KernelResult<T>;
    
    /// Convert None to NotFound error
    fn ok_or_not_found(self) -> KernelResult<T>;
    
    /// Convert None to BadAddress error
    fn ok_or_bad_addr(self, addr: usize) -> KernelResult<T>;
}

impl<T> OptionExt<T> for Option<T> {
    #[inline]
    fn ok_or_kernel_err(self, err: KernelError) -> KernelResult<T> {
        self.ok_or(err)
    }
    
    #[inline]
    fn ok_or_bad_fd(self, fd: usize) -> KernelResult<T> {
        self.ok_or(KernelError::BadFileDescriptor { fd })
    }
    
    #[inline]
    fn ok_or_not_found(self) -> KernelResult<T> {
        self.ok_or(KernelError::NotFound { path_hint: None })
    }
    
    #[inline]
    fn ok_or_bad_addr(self, addr: usize) -> KernelResult<T> {
        self.ok_or(KernelError::BadAddress { addr })
    }
}

/// Extension trait for Result to convert error types
pub trait ResultExt<T, E> {
    /// Map error to KernelError with custom mapping function
    fn map_kernel_err<F>(self, f: F) -> KernelResult<T>
    where
        F: FnOnce(E) -> KernelError;
    
    /// Convert raw errno result to KernelResult
    fn from_errno_result(self) -> KernelResult<T>
    where
        E: Into<isize>;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    #[inline]
    fn map_kernel_err<F>(self, f: F) -> KernelResult<T>
    where
        F: FnOnce(E) -> KernelError,
    {
        self.map_err(f)
    }
    
    #[inline]
    fn from_errno_result(self) -> KernelResult<T>
    where
        E: Into<isize>,
    {
        self.map_err(|e| KernelError::Errno(e.into()))
    }
}

/// Macro to return early with an error if condition is not met
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $err:expr) => {
        if !$cond {
            return Err($err);
        }
    };
}

/// Macro to return early with EINVAL if condition is not met
#[macro_export]
macro_rules! ensure_valid {
    ($cond:expr, $arg:literal) => {
        if !$cond {
            return Err($crate::utils::kerror::KernelError::InvalidArgument { arg_name: $arg });
        }
    };
}

/// Macro to unwrap or return with kernel error
#[macro_export]
macro_rules! try_or {
    ($opt:expr, $err:expr) => {
        match $opt {
            Some(v) => v,
            None => return Err($err),
        }
    };
}
