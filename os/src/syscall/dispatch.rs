//! System call dispatch table
//!
//! This module implements a function pointer table for syscall dispatch,
//! replacing the traditional large match statement approach.
//!
//! # Advantages
//!
//! 1. **Cache efficiency**: Dispatch is O(1) array lookup
//! 2. **Extensibility**: Adding syscalls requires only table entry
//! 3. **Modularity**: Handlers are decoupled from dispatch logic
//! 4. **Uniformity**: All handlers have the same signature
//!
//! # Handler Signature
//!
//! All syscall handlers use the signature:
//! ```rust
//! fn(args: &SyscallArgs) -> isize
//! ```
//!
//! Where `SyscallArgs` provides type-safe access to the 6 arguments.

use super::errno;
use super::fs::*;
use super::net::*;
use super::process::*;
use super::syscall_id::*;
use crate::fs::poll::FdSet;
use crate::task::Rusage;
use crate::timer::{ITimerVal, TimeSpec, Times};

/// Maximum syscall number supported
pub const MAX_SYSCALL_NR: usize = 512;

/// Syscall argument wrapper for type-safe access
#[derive(Clone, Copy)]
pub struct SyscallArgs {
    /// Raw argument values
    args: [usize; 6],
}

impl SyscallArgs {
    /// Create new syscall arguments from raw array
    #[inline]
    pub const fn new(args: [usize; 6]) -> Self {
        Self { args }
    }

    /// Get argument as usize
    #[inline]
    pub const fn arg(&self, idx: usize) -> usize {
        self.args[idx]
    }

    /// Get argument as isize
    #[inline]
    pub const fn arg_isize(&self, idx: usize) -> isize {
        self.args[idx] as isize
    }

    /// Get argument as u32
    #[inline]
    pub const fn arg_u32(&self, idx: usize) -> u32 {
        self.args[idx] as u32
    }

    /// Get argument as i32
    #[inline]
    pub const fn arg_i32(&self, idx: usize) -> i32 {
        self.args[idx] as i32
    }

    /// Get argument as pointer
    #[inline]
    pub const fn arg_ptr<T>(&self, idx: usize) -> *const T {
        self.args[idx] as *const T
    }

    /// Get argument as mutable pointer
    #[inline]
    pub const fn arg_mut_ptr<T>(&self, idx: usize) -> *mut T {
        self.args[idx] as *mut T
    }

    /// Get raw array reference
    #[inline]
    pub const fn raw(&self) -> &[usize; 6] {
        &self.args
    }
}

/// Type alias for syscall handler function
pub type SyscallHandler = fn(&SyscallArgs) -> isize;

/// Entry in the syscall table
#[derive(Clone, Copy)]
pub struct SyscallEntry {
    /// Handler function (None if not implemented)
    pub handler: Option<SyscallHandler>,
    /// Syscall name for debugging
    pub name: &'static str,
}

impl SyscallEntry {
    /// Create a new syscall entry
    pub const fn new(handler: SyscallHandler, name: &'static str) -> Self {
        Self {
            handler: Some(handler),
            name,
        }
    }

    /// Create an empty (unimplemented) entry
    pub const fn unimplemented(name: &'static str) -> Self {
        Self {
            handler: None,
            name,
        }
    }

    /// Create placeholder for unused syscall number
    pub const fn unused() -> Self {
        Self {
            handler: None,
            name: "unused",
        }
    }
}

// ============================================================================
// Wrapper functions that adapt existing syscall implementations
// ============================================================================

fn wrap_getcwd(a: &SyscallArgs) -> isize {
    sys_getcwd(a.arg(0), a.arg(1))
}

fn wrap_dup(a: &SyscallArgs) -> isize {
    sys_dup(a.arg(0))
}

fn wrap_dup2(a: &SyscallArgs) -> isize {
    sys_dup2(a.arg(0), a.arg(1))
}

fn wrap_dup3(a: &SyscallArgs) -> isize {
    sys_dup3(a.arg(0), a.arg(1), a.arg_u32(2))
}

fn wrap_fcntl(a: &SyscallArgs) -> isize {
    sys_fcntl(a.arg(0), a.arg_u32(1), a.arg(2))
}

fn wrap_ioctl(a: &SyscallArgs) -> isize {
    sys_ioctl(a.arg(0), a.arg_u32(1), a.arg(2))
}

fn wrap_mkdirat(a: &SyscallArgs) -> isize {
    sys_mkdirat(a.arg(0), a.arg_ptr(1), a.arg_u32(2))
}

fn wrap_unlinkat(a: &SyscallArgs) -> isize {
    sys_unlinkat(a.arg(0), a.arg_ptr(1), a.arg_u32(2))
}

fn wrap_umount2(a: &SyscallArgs) -> isize {
    sys_umount2(a.arg_ptr(0), a.arg_u32(1))
}

fn wrap_mount(a: &SyscallArgs) -> isize {
    sys_mount(a.arg_ptr(0), a.arg_ptr(1), a.arg_ptr(2), a.arg(3), a.arg_ptr(4))
}

fn wrap_statfs(a: &SyscallArgs) -> isize {
    sys_statfs(a.arg_ptr(0), a.arg_mut_ptr(1))
}

fn wrap_ftruncate(a: &SyscallArgs) -> isize {
    sys_ftruncate(a.arg(0), a.arg_isize(1))
}

fn wrap_faccessat(a: &SyscallArgs) -> isize {
    sys_faccessat2(a.arg(0), a.arg_ptr(1), a.arg_u32(2), 0u32)
}

fn wrap_chdir(a: &SyscallArgs) -> isize {
    sys_chdir(a.arg_ptr(0))
}

fn wrap_fchmodat(_a: &SyscallArgs) -> isize {
    sys_fchmodat()
}

fn wrap_openat(a: &SyscallArgs) -> isize {
    sys_openat(a.arg(0), a.arg_ptr(1), a.arg_u32(2), a.arg_u32(3))
}

fn wrap_close(a: &SyscallArgs) -> isize {
    sys_close(a.arg(0))
}

fn wrap_pipe2(a: &SyscallArgs) -> isize {
    sys_pipe2(a.arg(0), a.arg_u32(1))
}

fn wrap_getdents64(a: &SyscallArgs) -> isize {
    sys_getdents64(a.arg(0), a.arg_mut_ptr(1), a.arg(2))
}

fn wrap_lseek(a: &SyscallArgs) -> isize {
    sys_lseek(a.arg(0), a.arg_isize(1), a.arg_u32(2))
}

fn wrap_read(a: &SyscallArgs) -> isize {
    sys_read(a.arg(0), a.arg(1), a.arg(2))
}

fn wrap_write(a: &SyscallArgs) -> isize {
    sys_write(a.arg(0), a.arg(1), a.arg(2))
}

fn wrap_readv(a: &SyscallArgs) -> isize {
    sys_readv(a.arg(0), a.arg(1), a.arg(2))
}

fn wrap_writev(a: &SyscallArgs) -> isize {
    sys_writev(a.arg(0), a.arg(1), a.arg(2))
}

fn wrap_pread(a: &SyscallArgs) -> isize {
    sys_pread(a.arg(0), a.arg(1), a.arg(2), a.arg(3))
}

fn wrap_pwrite(a: &SyscallArgs) -> isize {
    sys_pwrite(a.arg(0), a.arg(1), a.arg(2), a.arg(3))
}

fn wrap_sendfile(a: &SyscallArgs) -> isize {
    sys_sendfile(a.arg(0), a.arg(1), a.arg_mut_ptr(2), a.arg(3))
}

fn wrap_pselect6(a: &SyscallArgs) -> isize {
    sys_pselect(
        a.arg(0),
        a.arg_mut_ptr::<FdSet>(1),
        a.arg_mut_ptr::<FdSet>(2),
        a.arg_mut_ptr::<FdSet>(3),
        a.arg_mut_ptr::<TimeSpec>(4),
        a.arg_ptr::<crate::task::Signals>(5),
    )
}

fn wrap_ppoll(a: &SyscallArgs) -> isize {
    sys_ppoll(a.arg(0), a.arg(1), a.arg(2), a.arg(3))
}

fn wrap_splice(a: &SyscallArgs) -> isize {
    sys_splice(
        a.arg(0),
        a.arg_mut_ptr(1),
        a.arg(2),
        a.arg_mut_ptr(3),
        a.arg(4),
        a.arg_u32(5),
    )
}

fn wrap_readlinkat(a: &SyscallArgs) -> isize {
    sys_readlinkat(a.arg(0), a.arg_ptr(1), a.arg_mut_ptr(2), a.arg(3))
}

fn wrap_fstatat(a: &SyscallArgs) -> isize {
    sys_fstatat(a.arg(0), a.arg_ptr(1), a.arg_mut_ptr(2), a.arg_u32(3))
}

fn wrap_fstat(a: &SyscallArgs) -> isize {
    sys_fstat(a.arg(0), a.arg_mut_ptr(1))
}

fn wrap_fsync(a: &SyscallArgs) -> isize {
    sys_fsync(a.arg(0))
}

fn wrap_utimensat(a: &SyscallArgs) -> isize {
    sys_utimensat(a.arg(0), a.arg_ptr(1), a.arg_ptr(2), a.arg_u32(3))
}

fn wrap_exit(a: &SyscallArgs) -> isize {
    sys_exit(a.arg_u32(0))
}

fn wrap_exit_group(a: &SyscallArgs) -> isize {
    sys_exit_group(a.arg_u32(0))
}

fn wrap_set_tid_address(a: &SyscallArgs) -> isize {
    sys_set_tid_address(a.arg(0))
}

fn wrap_futex(a: &SyscallArgs) -> isize {
    sys_futex(
        a.arg_mut_ptr(0),
        a.arg_u32(1),
        a.arg_u32(2),
        a.arg_ptr(3),
        a.arg_mut_ptr(4),
        a.arg_u32(5),
    )
}

fn wrap_set_robust_list(a: &SyscallArgs) -> isize {
    sys_set_robust_list(a.arg(0), a.arg(1))
}

fn wrap_get_robust_list(a: &SyscallArgs) -> isize {
    sys_get_robust_list(a.arg_u32(0), a.arg_mut_ptr(1), a.arg_mut_ptr(2))
}

fn wrap_nanosleep(a: &SyscallArgs) -> isize {
    sys_nanosleep(a.arg_ptr(0), a.arg_mut_ptr(1))
}

fn wrap_setitimer(a: &SyscallArgs) -> isize {
    sys_setitimer(a.arg(0), a.arg_ptr(1), a.arg_mut_ptr(2))
}

fn wrap_clock_gettime(a: &SyscallArgs) -> isize {
    sys_clock_gettime(a.arg(0), a.arg_mut_ptr(1))
}

fn wrap_clock_nanosleep(a: &SyscallArgs) -> isize {
    sys_clock_nanosleep(a.arg(0), a.arg_u32(1), a.arg_ptr(2), a.arg_mut_ptr(3))
}

fn wrap_syslog(a: &SyscallArgs) -> isize {
    sys_syslog(a.arg_u32(0), a.arg_mut_ptr(1), a.arg_u32(2))
}

fn wrap_yield(_a: &SyscallArgs) -> isize {
    sys_yield()
}

fn wrap_kill(a: &SyscallArgs) -> isize {
    sys_kill(a.arg(0), a.arg(1))
}

fn wrap_tkill(a: &SyscallArgs) -> isize {
    sys_tkill(a.arg(0), a.arg(1))
}

fn wrap_tgkill(a: &SyscallArgs) -> isize {
    sys_tgkill(a.arg(0), a.arg(1), a.arg(2))
}

fn wrap_sigaction(a: &SyscallArgs) -> isize {
    sys_sigaction(a.arg(0), a.arg(1), a.arg(2))
}

fn wrap_sigprocmask(a: &SyscallArgs) -> isize {
    sys_sigprocmask(a.arg_u32(0), a.arg(1), a.arg(2))
}

fn wrap_sigtimedwait(a: &SyscallArgs) -> isize {
    sys_sigtimedwait(a.arg(0), a.arg(1), a.arg(2))
}

fn wrap_sigreturn(_a: &SyscallArgs) -> isize {
    sys_sigreturn()
}

fn wrap_setpriority(a: &SyscallArgs) -> isize {
    sys_setpriority(a.arg_i32(0), a.arg_i32(1), a.arg_i32(2))
}

fn wrap_getpriority(a: &SyscallArgs) -> isize {
    sys_getpriority(a.arg_i32(0), a.arg_i32(1))
}

fn wrap_times(a: &SyscallArgs) -> isize {
    sys_times(a.arg_mut_ptr(0))
}

fn wrap_setpgid(a: &SyscallArgs) -> isize {
    sys_setpgid(a.arg(0), a.arg(1))
}

fn wrap_getpgid(a: &SyscallArgs) -> isize {
    sys_getpgid(a.arg(0))
}

fn wrap_setsid(_a: &SyscallArgs) -> isize {
    sys_setsid()
}

fn wrap_uname(a: &SyscallArgs) -> isize {
    sys_uname(a.arg_mut_ptr(0))
}

fn wrap_getrusage(a: &SyscallArgs) -> isize {
    sys_getrusage(a.arg_isize(0), a.arg_mut_ptr(1))
}

fn wrap_umask(a: &SyscallArgs) -> isize {
    sys_umask(a.arg_u32(0))
}

fn wrap_gettimeofday(a: &SyscallArgs) -> isize {
    sys_gettimeofday(a.arg_mut_ptr(0), a.arg_mut_ptr(1))
}

fn wrap_getpid(_a: &SyscallArgs) -> isize {
    sys_getpid()
}

fn wrap_getppid(_a: &SyscallArgs) -> isize {
    sys_getppid()
}

fn wrap_getuid(_a: &SyscallArgs) -> isize {
    sys_getuid()
}

fn wrap_geteuid(_a: &SyscallArgs) -> isize {
    sys_geteuid()
}

fn wrap_getgid(_a: &SyscallArgs) -> isize {
    sys_getgid()
}

fn wrap_getegid(_a: &SyscallArgs) -> isize {
    sys_getegid()
}

fn wrap_gettid(_a: &SyscallArgs) -> isize {
    sys_gettid()
}

fn wrap_sysinfo(a: &SyscallArgs) -> isize {
    sys_sysinfo(a.arg_mut_ptr(0))
}

fn wrap_socket(a: &SyscallArgs) -> isize {
    sys_socket(a.arg_u32(0), a.arg_u32(1), a.arg_u32(2))
}

fn wrap_socketpair(a: &SyscallArgs) -> isize {
    sys_socketpair(a.arg_u32(0), a.arg_u32(1), a.arg_u32(2), a.arg(3))
}

fn wrap_bind(a: &SyscallArgs) -> isize {
    sys_bind(a.arg_u32(0), a.arg(1), a.arg_u32(2))
}

fn wrap_listen(a: &SyscallArgs) -> isize {
    sys_listen(a.arg_u32(0), a.arg_u32(1))
}

fn wrap_accept(a: &SyscallArgs) -> isize {
    sys_accept(a.arg_u32(0), a.arg(1), a.arg(2))
}

fn wrap_connect(a: &SyscallArgs) -> isize {
    sys_connect(a.arg_u32(0), a.arg(1), a.arg_u32(2))
}

fn wrap_getsockname(a: &SyscallArgs) -> isize {
    sys_getsockname(a.arg_u32(0), a.arg(1), a.arg(2))
}

fn wrap_getpeername(a: &SyscallArgs) -> isize {
    sys_getpeername(a.arg_u32(0), a.arg(1), a.arg(2))
}

fn wrap_sendto(a: &SyscallArgs) -> isize {
    sys_sendto(a.arg_u32(0), a.arg(1), a.arg(2), a.arg_u32(3), a.arg(4), a.arg_u32(5))
}

fn wrap_recvfrom(a: &SyscallArgs) -> isize {
    sys_recvfrom(a.arg_u32(0), a.arg(1), a.arg_u32(2), a.arg_u32(3), a.arg(4), a.arg(5))
}

fn wrap_setsockopt(a: &SyscallArgs) -> isize {
    sys_setsockopt(a.arg_u32(0), a.arg_u32(1), a.arg_u32(2), a.arg(3), a.arg_u32(4))
}

fn wrap_getsockopt(a: &SyscallArgs) -> isize {
    sys_getsockopt(a.arg_u32(0), a.arg_u32(1), a.arg_u32(2), a.arg(3), a.arg(4))
}

fn wrap_sock_shutdown(a: &SyscallArgs) -> isize {
    sys_sock_shutdown(a.arg_u32(0), a.arg_u32(1))
}

fn wrap_sbrk(a: &SyscallArgs) -> isize {
    sys_sbrk(a.arg_isize(0))
}

fn wrap_brk(a: &SyscallArgs) -> isize {
    sys_brk(a.arg(0))
}

fn wrap_munmap(a: &SyscallArgs) -> isize {
    sys_munmap(a.arg(0), a.arg(1))
}

fn wrap_clone(a: &SyscallArgs) -> isize {
    sys_clone(a.arg_u32(0), a.arg_ptr(1), a.arg_mut_ptr(2), a.arg(3), a.arg_mut_ptr(4))
}

fn wrap_execve(a: &SyscallArgs) -> isize {
    sys_execve(a.arg_ptr(0), a.arg_ptr(1), a.arg_ptr(2))
}

fn wrap_mmap(a: &SyscallArgs) -> isize {
    sys_mmap(a.arg(0), a.arg(1), a.arg(2), a.arg(3), a.arg(4), a.arg(5))
}

fn wrap_mprotect(a: &SyscallArgs) -> isize {
    sys_mprotect(a.arg(0), a.arg(1), a.arg(2))
}

fn wrap_msync(a: &SyscallArgs) -> isize {
    sys_msync(a.arg(0), a.arg(1), a.arg_u32(2))
}

fn wrap_madvise(a: &SyscallArgs) -> isize {
    sys_madvise(a.arg(0), a.arg(1), a.arg_u32(2))
}

fn wrap_wait4(a: &SyscallArgs) -> isize {
    sys_wait4(a.arg_isize(0), a.arg_mut_ptr(1), a.arg_u32(2), a.arg_mut_ptr(3))
}

fn wrap_prlimit(a: &SyscallArgs) -> isize {
    sys_prlimit(a.arg(0), a.arg_u32(1), a.arg_ptr(2), a.arg_mut_ptr(3))
}

fn wrap_renameat2(a: &SyscallArgs) -> isize {
    sys_renameat2(a.arg(0), a.arg_ptr(1), a.arg(2), a.arg_ptr(3), a.arg_u32(4))
}

fn wrap_getrandom(a: &SyscallArgs) -> isize {
    super::sys_getrandom(a.arg(0), a.arg(1), a.arg_u32(2))
}

fn wrap_membarrier(a: &SyscallArgs) -> isize {
    sys_memorybarrier(a.arg(0), a.arg(1), a.arg(2))
}

fn wrap_copy_file_range(a: &SyscallArgs) -> isize {
    sys_copy_file_range(a.arg(0), a.arg_mut_ptr(1), a.arg(2), a.arg_mut_ptr(3), a.arg(4), a.arg_u32(5))
}

fn wrap_statx(a: &SyscallArgs) -> isize {
    sys_statx(a.arg(0), a.arg_ptr(1), a.arg_u32(2), a.arg_u32(3), a.arg_mut_ptr(4))
}

fn wrap_faccessat2(a: &SyscallArgs) -> isize {
    sys_faccessat2(a.arg(0), a.arg_ptr(1), a.arg_u32(2), a.arg_u32(3))
}

fn wrap_close_range(a: &SyscallArgs) -> isize {
    sys_close_range(a.arg_u32(0), a.arg_u32(1), a.arg_u32(2))
}

fn wrap_eventfd2(a: &SyscallArgs) -> isize {
    sys_eventfd2(a.arg_u32(0), a.arg_i32(1))
}

fn wrap_waitid(a: &SyscallArgs) -> isize {
    sys_waitid(a.arg_u32(0), a.arg_u32(1), a.arg_mut_ptr(2), a.arg_u32(3))
}

fn wrap_shutdown(_a: &SyscallArgs) -> isize {
    sys_shutdown()
}

fn wrap_get_time(_a: &SyscallArgs) -> isize {
    sys_get_time()
}

fn wrap_open(a: &SyscallArgs) -> isize {
    sys_openat(AT_FDCWD, a.arg_ptr(0), a.arg_u32(1), 0o777u32)
}

// ============================================================================
// Syscall table construction
// ============================================================================

/// Build the syscall dispatch table
///
/// This function constructs a static table mapping syscall numbers to handlers.
/// The table is initialized at compile time for efficiency.
pub fn dispatch_syscall(id: usize, args: [usize; 6]) -> Option<(&'static str, isize)> {
    let syscall_args = SyscallArgs::new(args);
    
    let (name, handler): (&'static str, Option<SyscallHandler>) = match id {
        SYSCALL_GETCWD => ("getcwd", Some(wrap_getcwd)),
        SYSCALL_DUP => ("dup", Some(wrap_dup)),
        SYSCALL_DUP2 => ("dup2", Some(wrap_dup2)),
        SYSCALL_DUP3 => ("dup3", Some(wrap_dup3)),
        SYSCALL_FCNTL => ("fcntl", Some(wrap_fcntl)),
        SYSCALL_IOCTL => ("ioctl", Some(wrap_ioctl)),
        SYSCALL_MKDIRAT => ("mkdirat", Some(wrap_mkdirat)),
        SYSCALL_UNLINKAT => ("unlinkat", Some(wrap_unlinkat)),
        SYSCALL_UMOUNT2 => ("umount2", Some(wrap_umount2)),
        SYSCALL_MOUNT => ("mount", Some(wrap_mount)),
        SYSCALL_STATFS => ("statfs", Some(wrap_statfs)),
        SYSCALL_FTRUNCATE => ("ftruncate", Some(wrap_ftruncate)),
        SYSCALL_FACCESSAT => ("faccessat", Some(wrap_faccessat)),
        SYSCALL_CHDIR => ("chdir", Some(wrap_chdir)),
        SYSCALL_FCHMODAT => ("fchmodat", Some(wrap_fchmodat)),
        SYSCALL_OPENAT => ("openat", Some(wrap_openat)),
        SYSCALL_CLOSE => ("close", Some(wrap_close)),
        SYSCALL_PIPE2 => ("pipe2", Some(wrap_pipe2)),
        SYSCALL_GETDENTS64 => ("getdents64", Some(wrap_getdents64)),
        SYSCALL_LSEEK => ("lseek", Some(wrap_lseek)),
        SYSCALL_READ => ("read", Some(wrap_read)),
        SYSCALL_WRITE => ("write", Some(wrap_write)),
        SYSCALL_READV => ("readv", Some(wrap_readv)),
        SYSCALL_WRITEV => ("writev", Some(wrap_writev)),
        SYSCALL_PREAD => ("pread", Some(wrap_pread)),
        SYSCALL_PWRITE => ("pwrite", Some(wrap_pwrite)),
        SYSCALL_SENDFILE => ("sendfile", Some(wrap_sendfile)),
        SYSCALL_PSELECT6 => ("pselect6", Some(wrap_pselect6)),
        SYSCALL_PPOLL => ("ppoll", Some(wrap_ppoll)),
        SYSCALL_SPLICE => ("splice", Some(wrap_splice)),
        SYSCALL_READLINKAT => ("readlinkat", Some(wrap_readlinkat)),
        SYSCALL_FSTATAT => ("fstatat", Some(wrap_fstatat)),
        SYSCALL_FSTAT => ("fstat", Some(wrap_fstat)),
        SYSCALL_FSYNC => ("fsync", Some(wrap_fsync)),
        SYSCALL_UTIMENSAT => ("utimensat", Some(wrap_utimensat)),
        SYSCALL_EXIT => ("exit", Some(wrap_exit)),
        SYSCALL_EXIT_GROUP => ("exit_group", Some(wrap_exit_group)),
        SYSCALL_SET_TID_ADDRESS => ("set_tid_address", Some(wrap_set_tid_address)),
        SYSCALL_FUTEX => ("futex", Some(wrap_futex)),
        SYSCALL_SET_ROBUST_LIST => ("set_robust_list", Some(wrap_set_robust_list)),
        SYSCALL_GET_ROBUST_LIST => ("get_robust_list", Some(wrap_get_robust_list)),
        SYSCALL_NANOSLEEP => ("nanosleep", Some(wrap_nanosleep)),
        SYSCALL_SETITIMER => ("setitimer", Some(wrap_setitimer)),
        SYSCALL_CLOCK_GETTIME => ("clock_gettime", Some(wrap_clock_gettime)),
        SYSCALL_CLOCK_NANOSLEEP => ("clock_nanosleep", Some(wrap_clock_nanosleep)),
        SYSCALL_SYSLOG => ("syslog", Some(wrap_syslog)),
        SYSCALL_YIELD => ("yield", Some(wrap_yield)),
        SYSCALL_KILL => ("kill", Some(wrap_kill)),
        SYSCALL_TKILL => ("tkill", Some(wrap_tkill)),
        SYSCALL_TGKILL => ("tgkill", Some(wrap_tgkill)),
        SYSCALL_SIGACTION => ("sigaction", Some(wrap_sigaction)),
        SYSCALL_SIGPROCMASK => ("sigprocmask", Some(wrap_sigprocmask)),
        SYSCALL_SIGTIMEDWAIT => ("sigtimedwait", Some(wrap_sigtimedwait)),
        SYSCALL_SIGRETURN => ("sigreturn", Some(wrap_sigreturn)),
        SYSCALL_SETPRIORITY => ("setpriority", Some(wrap_setpriority)),
        SYSCALL_GETPRIORITY => ("getpriority", Some(wrap_getpriority)),
        SYSCALL_TIMES => ("times", Some(wrap_times)),
        SYSCALL_SETPGID => ("setpgid", Some(wrap_setpgid)),
        SYSCALL_GETPGID => ("getpgid", Some(wrap_getpgid)),
        SYSCALL_SETSID => ("setsid", Some(wrap_setsid)),
        SYSCALL_UNAME => ("uname", Some(wrap_uname)),
        SYSCALL_GETRUSAGE => ("getrusage", Some(wrap_getrusage)),
        SYSCALL_UMASK => ("umask", Some(wrap_umask)),
        SYSCALL_GET_TIME_OF_DAY => ("gettimeofday", Some(wrap_gettimeofday)),
        SYSCALL_GETPID => ("getpid", Some(wrap_getpid)),
        SYSCALL_GETPPID => ("getppid", Some(wrap_getppid)),
        SYSCALL_GETUID => ("getuid", Some(wrap_getuid)),
        SYSCALL_GETEUID => ("geteuid", Some(wrap_geteuid)),
        SYSCALL_GETGID => ("getgid", Some(wrap_getgid)),
        SYSCALL_GETEGID => ("getegid", Some(wrap_getegid)),
        SYSCALL_GETTID => ("gettid", Some(wrap_gettid)),
        SYSCALL_SYSINFO => ("sysinfo", Some(wrap_sysinfo)),
        SYSCALL_SOCKET => ("socket", Some(wrap_socket)),
        SYSCALL_SOCKETPAIR => ("socketpair", Some(wrap_socketpair)),
        SYSCALL_BIND => ("bind", Some(wrap_bind)),
        SYSCALL_LISTEN => ("listen", Some(wrap_listen)),
        SYSCALL_ACCEPT => ("accept", Some(wrap_accept)),
        SYSCALL_CONNECT => ("connect", Some(wrap_connect)),
        SYSCALL_GETSOCKNAME => ("getsockname", Some(wrap_getsockname)),
        SYSCALL_GETPEERNAME => ("getpeername", Some(wrap_getpeername)),
        SYSCALL_SENDTO => ("sendto", Some(wrap_sendto)),
        SYSCALL_RECVFROM => ("recvfrom", Some(wrap_recvfrom)),
        SYSCALL_SETSOCKOPT => ("setsockopt", Some(wrap_setsockopt)),
        SYSCALL_GETSOCKOPT => ("getsockopt", Some(wrap_getsockopt)),
        SYSCALL_SOCK_SHUTDOWN => ("sock_shutdown", Some(wrap_sock_shutdown)),
        SYSCALL_SBRK => ("sbrk", Some(wrap_sbrk)),
        SYSCALL_BRK => ("brk", Some(wrap_brk)),
        SYSCALL_MUNMAP => ("munmap", Some(wrap_munmap)),
        SYSCALL_CLONE => ("clone", Some(wrap_clone)),
        SYSCALL_EXECVE => ("execve", Some(wrap_execve)),
        SYSCALL_MMAP => ("mmap", Some(wrap_mmap)),
        SYSCALL_MPROTECT => ("mprotect", Some(wrap_mprotect)),
        SYSCALL_MSYNC => ("msync", Some(wrap_msync)),
        SYSCALL_MADVISE => ("madvise", Some(wrap_madvise)),
        SYSCALL_WAIT4 => ("wait4", Some(wrap_wait4)),
        SYSCALL_PRLIMIT => ("prlimit", Some(wrap_prlimit)),
        SYSCALL_RENAMEAT2 => ("renameat2", Some(wrap_renameat2)),
        SYSCALL_GETRANDOM => ("getrandom", Some(wrap_getrandom)),
        SYSCALL_MEMBARRIER => ("membarrier", Some(wrap_membarrier)),
        SYSCALL_COPY_FILE_RANGE => ("copy_file_range", Some(wrap_copy_file_range)),
        SYSCALL_STATX => ("statx", Some(wrap_statx)),
        SYSCALL_FACCESSAT2 => ("faccessat2", Some(wrap_faccessat2)),
        SYSCALL_CLOSE_RANGE => ("close_range", Some(wrap_close_range)),
        SYSCALL_EVENTFD2 => ("eventfd2", Some(wrap_eventfd2)),
        SYSCALL_WAITID => ("waitid", Some(wrap_waitid)),
        // Non-standard syscalls
        SYSCALL_SHUTDOWN => ("shutdown", Some(wrap_shutdown)),
        SYSCALL_GET_TIME => ("get_time", Some(wrap_get_time)),
        SYSCALL_OPEN => ("open", Some(wrap_open)),
        _ => ("unknown", None),
    };
    
    handler.map(|h| (name, h(&syscall_args)))
}

/// Get syscall name from ID (for logging)
pub fn get_syscall_name(id: usize) -> &'static str {
    match id {
        SYSCALL_GETCWD => "getcwd",
        SYSCALL_DUP => "dup",
        SYSCALL_DUP2 => "dup2",
        SYSCALL_DUP3 => "dup3",
        SYSCALL_FCNTL => "fcntl",
        SYSCALL_IOCTL => "ioctl",
        SYSCALL_MKDIRAT => "mkdirat",
        SYSCALL_UNLINKAT => "unlinkat",
        SYSCALL_UMOUNT2 => "umount2",
        SYSCALL_MOUNT => "mount",
        SYSCALL_STATFS => "statfs",
        SYSCALL_FTRUNCATE => "ftruncate",
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
        SYSCALL_PSELECT6 => "pselect6",
        SYSCALL_PPOLL => "ppoll",
        SYSCALL_SPLICE => "splice",
        SYSCALL_READLINKAT => "readlinkat",
        SYSCALL_FSTATAT => "fstatat",
        SYSCALL_FSTAT => "fstat",
        SYSCALL_FSYNC => "fsync",
        SYSCALL_UTIMENSAT => "utimensat",
        SYSCALL_EXIT => "exit",
        SYSCALL_EXIT_GROUP => "exit_group",
        SYSCALL_SET_TID_ADDRESS => "set_tid_address",
        SYSCALL_FUTEX => "futex",
        SYSCALL_SET_ROBUST_LIST => "set_robust_list",
        SYSCALL_GET_ROBUST_LIST => "get_robust_list",
        SYSCALL_NANOSLEEP => "nanosleep",
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
        SYSCALL_GET_TIME_OF_DAY => "gettimeofday",
        SYSCALL_GETPID => "getpid",
        SYSCALL_GETPPID => "getppid",
        SYSCALL_GETUID => "getuid",
        SYSCALL_GETEUID => "geteuid",
        SYSCALL_GETGID => "getgid",
        SYSCALL_GETEGID => "getegid",
        SYSCALL_GETTID => "gettid",
        SYSCALL_SYSINFO => "sysinfo",
        SYSCALL_SOCKET => "socket",
        SYSCALL_SOCKETPAIR => "socketpair",
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
        SYSCALL_SOCK_SHUTDOWN => "sock_shutdown",
        SYSCALL_SBRK => "sbrk",
        SYSCALL_BRK => "brk",
        SYSCALL_MUNMAP => "munmap",
        SYSCALL_CLONE => "clone",
        SYSCALL_EXECVE => "execve",
        SYSCALL_MMAP => "mmap",
        SYSCALL_MPROTECT => "mprotect",
        SYSCALL_MSYNC => "msync",
        SYSCALL_MADVISE => "madvise",
        SYSCALL_WAIT4 => "wait4",
        SYSCALL_PRLIMIT => "prlimit",
        SYSCALL_RENAMEAT2 => "renameat2",
        SYSCALL_GETRANDOM => "getrandom",
        SYSCALL_MEMBARRIER => "membarrier",
        SYSCALL_COPY_FILE_RANGE => "copy_file_range",
        SYSCALL_STATX => "statx",
        SYSCALL_FACCESSAT2 => "faccessat2",
        SYSCALL_CLOSE_RANGE => "close_range",
        SYSCALL_EVENTFD2 => "eventfd2",
        SYSCALL_WAITID => "waitid",
        SYSCALL_SHUTDOWN => "shutdown",
        SYSCALL_GET_TIME => "get_time",
        SYSCALL_OPEN => "open",
        _ => "unknown",
    }
}
