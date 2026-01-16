//! System call handling and dispatch
//!
//! This module implements the kernel's system call interface, providing:
//! - Unified syscall dispatch mechanism via function pointer table
//! - Context abstraction for safe resource access
//! - Comprehensive error handling
//!
//! # Supported System Call Categories
//!
//! | Category | Examples | Module |
//! |----------|----------|--------|
//! | Filesystem | open, read, write, close | `fs` |
//! | Process | fork, exec, wait, exit | `process` |
//! | Memory | mmap, munmap, brk, mprotect | `process` |
//! | Network | socket, bind, connect | `net` |
//! | Signals | sigaction, kill, sigreturn | `process` |
//! | Time | clock_gettime, nanosleep | `process` |
//!
//! # Architecture
//!
//! System calls are dispatched through a function pointer table (`SYSCALL_HANDLERS`)
//! rather than a large match statement. This provides better cache locality and
//! enables easier extension.

#[macro_use]
mod syscall_macro;

pub mod context;
pub mod dispatch;
pub mod errno;
pub mod fs;
pub mod io_ops;
mod net;
mod process;
mod syscall_id;

use core::convert::TryFrom;
use fs::*;
use log::{error, info};
use net::*;
pub use process::CloneFlags;
use process::*;
use syscall_id::*;

/// Get system call name by ID
///
/// Returns a human-readable name for debugging and logging
pub fn syscall_name(id: usize) -> &'static str {
    match id {
        SYSCALL_DUP => "dup",
        SYSCALL_DUP2 => "dup2",
        SYSCALL_DUP3 => "dup3",
        SYSCALL_OPEN => "open",
        SYSCALL_GET_TIME => "get_time",
        SYSCALL_GETCWD => "getcwd",
        SYSCALL_FCNTL => "fcntl",
        SYSCALL_IOCTL => "ioctl",
        SYSCALL_MKDIRAT => "mkdirat",
        SYSCALL_UNLINKAT => "unlinkat",
        SYSCALL_LINKAT => "linkat",
        SYSCALL_UMOUNT2 => "umount2",
        SYSCALL_MOUNT => "mount",
        SYSCALL_FACCESSAT => "faccessat",
        SYSCALL_CHDIR => "chdir",
        SYSCALL_FCHMODAT => "fchmodat",
        SYSCALL_OPENAT => "openat",
        SYSCALL_CLOSE => "close",
        SYSCALL_PIPE2 => "pipe2",
        SYSCALL_GETDENTS64 => "getdents64",
        SYSCALL_LSEEK => "lseek",
        SYSCALL_READ => "read",
        SYSCALL_WRITE => "write",
        SYSCALL_READV => "readv",
        SYSCALL_WRITEV => "writev",
        SYSCALL_PREAD => "pread",
        SYSCALL_PWRITE => "pwrite",
        SYSCALL_SENDFILE => "sendfile",
        SYSCALL_SPLICE => "splice",
        SYSCALL_PSELECT6 => "pselect6",
        SYSCALL_PPOLL => "ppoll",
        SYSCALL_READLINKAT => "readlinkat",
        SYSCALL_FSTATAT => "fstatat",
        SYSCALL_FSTAT => "fstat",
        SYSCALL_STATFS => "statfs",
        SYSCALL_FTRUNCATE => "ftruncate",
        SYSCALL_FSYNC => "fsync",
        SYSCALL_UTIMENSAT => "utimensat",
        SYSCALL_EXIT => "exit",
        SYSCALL_EXIT_GROUP => "exit_GROUP",
        SYSCALL_SET_TID_ADDRESS => "set_tid_address",
        SYSCALL_FUTEX => "futex",
        SYSCALL_SET_ROBUST_LIST => "set_robust_list",
        SYSCALL_GET_ROBUST_LIST => "get_robust_list",
        SYSCALL_NANOSLEEP => "nanosleep",
        SYSCALL_GETITIMER => "getitimer",
        SYSCALL_SETITIMER => "setitimer",
        SYSCALL_CLOCK_GETTIME => "clock_gettime",
        SYSCALL_CLOCK_NANOSLEEP => "clock_nanosleep",
        SYSCALL_SYSLOG => "syslog",
        SYSCALL_YIELD => "yield",
        SYSCALL_KILL => "kill",
        SYSCALL_TKILL => "tkill",
        SYSCALL_TGKILL => "tgkill",
        SYSCALL_SIGACTION => "sigaction",
        SYSCALL_SIGPROCMASK => "sigprocmask",
        SYSCALL_SIGTIMEDWAIT => "sigtimedwait",
        SYSCALL_SIGRETURN => "sigreturn",
        SYSCALL_TIMES => "times",
        SYSCALL_SETPGID => "setpgid",
        SYSCALL_GETPGID => "getpgid",
        SYSCALL_SETSID => "setsid",
        SYSCALL_UNAME => "uname",
        SYSCALL_GETRUSAGE => "getrusage",
        SYSCALL_UMASK => "umask",
        SYSCALL_GET_TIME_OF_DAY => "get_time_of_day",
        SYSCALL_GETPID => "getpid",
        SYSCALL_GETPPID => "getppid",
        SYSCALL_GETUID => "getuid",
        SYSCALL_GETEUID => "geteuid",
        SYSCALL_GETGID => "getgid",
        SYSCALL_GETEGID => "getegid",
        SYSCALL_GETTID => "gettid",
        SYSCALL_SYSINFO => "sysinfo",
        SYSCALL_SOCKET => "socket",
        SYSCALL_BIND => "bind",
        SYSCALL_LISTEN => "listen",
        SYSCALL_ACCEPT => "accept",
        SYSCALL_CONNECT => "connect",
        SYSCALL_GETSOCKNAME => "getsockname",
        SYSCALL_GETPEERNAME => "getpeername",
        SYSCALL_SENDTO => "sendto",
        SYSCALL_RECVFROM => "recvfrom",
        SYSCALL_SETSOCKOPT => "setsockopt",
        SYSCALL_GETSOCKOPT => "getsockopt",
        SYSCALL_SBRK => "sbrk",
        SYSCALL_BRK => "brk",
        SYSCALL_MUNMAP => "munmap",
        SYSCALL_CLONE => "clone",
        SYSCALL_EXECVE => "execve",
        SYSCALL_MMAP => "mmap",
        SYSCALL_MPROTECT => "mprotect",
        SYSCALL_MSYNC => "msync",
        SYSCALL_WAIT4 => "wait4",
        SYSCALL_PRLIMIT => "prlimit",
        SYSCALL_RENAMEAT2 => "renameat2",
        SYSCALL_FACCESSAT2 => "faccessat2",
        SYSCALL_MEMBARRIER => "membarrier",
        SYSCALL_STATX => "statx",
        SYSCALL_GETRANDOM => "getrandom",
        SYSCALL_COPY_FILE_RANGE => "copy_file_range",
        // non-standard
        SYSCALL_LS => "ls",
        SYSCALL_SHUTDOWN => "shutdown",
        SYSCALL_CLEAR => "clear",
        _ => "unknown",
    }
}
use crate::syscall::errno::Errno;

/// Syscall blacklist for logging suppression
/// 
/// These syscalls are too frequent to log without flooding output
const SYSCALL_LOG_BLACKLIST: &[usize] = &[
    SYSCALL_YIELD,
    SYSCALL_WRITE,
    SYSCALL_GETDENTS64,
    SYSCALL_READV,
    SYSCALL_WRITEV,
    SYSCALL_PSELECT6,
    SYSCALL_SIGACTION,
    SYSCALL_SIGPROCMASK,
    SYSCALL_CLOCK_GETTIME,
];

/// Check if syscall should be logged
#[inline]
fn should_log_syscall(id: usize) -> bool {
    option_env!("LOG").is_some() && !SYSCALL_LOG_BLACKLIST.contains(&id)
}

/// Log syscall entry with arguments
fn log_syscall_entry(name: &str, id: usize, args: &[usize; 6]) {
    info!(
        "[syscall] {}({}) args: [{:X}, {:X}, {:X}, {:X}, {:X}, {:X}]",
        name, id, args[0], args[1], args[2], args[3], args[4], args[5]
    );
}

/// Log syscall exit with result
fn log_syscall_exit(name: &str, id: usize, ret: isize) {
    match Errno::try_from(ret) {
        Ok(errno) => info!("[syscall] {}({}) -> {:?}", name, id, errno),
        Err(val) => info!("[syscall] {}({}) -> {:X}", name, id, val.number),
    }
}

/// Handle unimplemented syscall
fn handle_unsupported_syscall(id: usize, args: &[usize; 6]) -> isize {
    let name = dispatch::get_syscall_name(id);
    println!("Unsupported syscall:{} ({})", name, id);
    error!("Unsupported syscall:{} ({}), calling over arguments:", name, id);
    for (idx, arg) in args.iter().enumerate() {
        error!("args[{}]: {:X}", idx, arg);
    }
    if let Some(task) = crate::task::current_task() {
        task.acquire_inner_lock().add_signal(crate::task::Signals::SIGSYS);
    }
    errno::ENOSYS
}

/// Main syscall dispatch entry point
///
/// This function serves as the kernel's primary syscall handler. It:
/// 1. Optionally logs the syscall entry (if LOG is enabled and syscall not blacklisted)
/// 2. Dispatches to the appropriate handler via the function pointer table
/// 3. Handles unsupported syscalls with proper error reporting
/// 4. Optionally logs the syscall exit
///
/// # Arguments
/// * `syscall_id` - The syscall number from user space
/// * `args` - Array of 6 syscall arguments
///
/// # Returns
/// * Positive/zero value on success (meaning depends on specific syscall)
/// * Negative errno on failure
pub fn syscall(syscall_id: usize, args: [usize; 6]) -> isize {
    let should_log = should_log_syscall(syscall_id);
    let name = dispatch::get_syscall_name(syscall_id);
    
    if should_log {
        log_syscall_entry(name, syscall_id, &args);
    }
    
    let ret = match dispatch::dispatch_syscall(syscall_id, args) {
        Some((_name, result)) => result,
        None => handle_unsupported_syscall(syscall_id, &args),
    };
    
    if should_log {
        log_syscall_exit(name, syscall_id, ret);
    }
    
    ret
}

/// Random number generation syscall (placeholder implementation)
///
/// TODO: Implement proper random number generation with entropy pool
pub fn sys_getrandom(_buf: usize, _buflen: usize, _flags: u32) -> isize {
    0
}
