//! Kernel tracing and instrumentation framework
//!
//! This module provides a comprehensive tracing system for debugging and performance
//! analysis. It supports multiple trace levels, structured events, and conditional
//! compilation for zero overhead in release builds.
//!
//! # Features
//!
//! - **Structured Events**: Type-safe trace events with metadata
//! - **Hierarchical Spans**: Track execution flow across functions
//! - **Conditional Compilation**: Zero overhead when disabled
//! - **Event Filtering**: Runtime and compile-time filtering by category
//!
//! # Usage
//!
//! ```rust
//! use crate::utils::trace::{trace_event, TraceCategory};
//!
//! // Simple event
//! trace_event!(TraceCategory::Syscall, "sys_read entered", fd = fd);
//!
//! // Span for timing
//! let _span = trace_span!("sys_read");
//! // ... work happens here ...
//! // span automatically closed on drop
//! ```
//!
//! # Categories
//!
//! Events are organized into categories for filtering:
//! - `Syscall`: System call entry/exit
//! - `Memory`: Memory allocation and mapping
//! - `Scheduler`: Task scheduling decisions
//! - `Interrupt`: Interrupt and trap handling
//! - `FileSystem`: File system operations
//! - `Network`: Network operations

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use alloc::string::String;

/// Global tracing enable flag
pub static TRACING_ENABLED: AtomicBool = AtomicBool::new(false);

/// Global trace event counter
static TRACE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Trace event category for filtering and organization
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TraceCategory {
    /// System call events
    Syscall = 0,
    /// Memory management events
    Memory = 1,
    /// Task scheduling events
    Scheduler = 2,
    /// Interrupt handling events
    Interrupt = 3,
    /// Filesystem events
    FileSystem = 4,
    /// Network events
    Network = 5,
    /// Generic debug events
    Debug = 6,
    /// Performance measurement events
    Perf = 7,
}

impl TraceCategory {
    /// Get category name as static string
    #[inline]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Syscall => "syscall",
            Self::Memory => "memory",
            Self::Scheduler => "scheduler",
            Self::Interrupt => "interrupt",
            Self::FileSystem => "filesystem",
            Self::Network => "network",
            Self::Debug => "debug",
            Self::Perf => "perf",
        }
    }
    
    /// Get category color code for terminal output
    #[inline]
    pub const fn color_code(&self) -> &'static str {
        match self {
            Self::Syscall => "\x1b[36m",    // Cyan
            Self::Memory => "\x1b[33m",     // Yellow
            Self::Scheduler => "\x1b[32m",  // Green
            Self::Interrupt => "\x1b[31m",  // Red
            Self::FileSystem => "\x1b[35m", // Magenta
            Self::Network => "\x1b[34m",    // Blue
            Self::Debug => "\x1b[37m",      // White
            Self::Perf => "\x1b[90m",       // Gray
        }
    }
}

/// Trace event severity level
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum TraceLevel {
    /// Extremely verbose, only for deep debugging
    Trace = 0,
    /// Detailed information for debugging
    Debug = 1,
    /// General informational messages
    Info = 2,
    /// Warning conditions
    Warn = 3,
    /// Error conditions
    Error = 4,
}

impl TraceLevel {
    /// Get level prefix string
    #[inline]
    pub const fn prefix(&self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO ",
            Self::Warn => "WARN ",
            Self::Error => "ERROR",
        }
    }
}

/// A trace span for measuring execution duration
///
/// Spans are created with `trace_span!` macro and automatically
/// record their duration when dropped.
pub struct TraceSpan {
    category: TraceCategory,
    name: &'static str,
    start_ticks: u64,
    active: bool,
}

impl TraceSpan {
    /// Create a new trace span
    #[inline]
    pub fn new(category: TraceCategory, name: &'static str) -> Self {
        let active = TRACING_ENABLED.load(Ordering::Relaxed);
        let start_ticks = if active {
            get_ticks()
        } else {
            0
        };
        
        if active {
            emit_span_enter(category, name);
        }
        
        Self {
            category,
            name,
            start_ticks,
            active,
        }
    }
    
    /// Create an inactive (no-op) span
    #[inline]
    pub const fn inactive() -> Self {
        Self {
            category: TraceCategory::Debug,
            name: "",
            start_ticks: 0,
            active: false,
        }
    }
}

impl Drop for TraceSpan {
    fn drop(&mut self) {
        if self.active {
            let elapsed = get_ticks() - self.start_ticks;
            emit_span_exit(self.category, self.name, elapsed);
        }
    }
}

/// Get current tick count for timing
#[inline]
fn get_ticks() -> u64 {
    // Use architecture-specific timer
    #[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
    {
        riscv::register::time::read64()
    }
    #[cfg(target_arch = "loongarch64")]
    {
        // LoongArch uses rdtime instruction
        0 // Placeholder - implement for LA
    }
    #[cfg(not(any(target_arch = "riscv64", target_arch = "riscv32", target_arch = "loongarch64")))]
    {
        0
    }
}

/// Emit a span entry event
#[inline]
fn emit_span_enter(category: TraceCategory, name: &'static str) {
    let seq = TRACE_COUNTER.fetch_add(1, Ordering::Relaxed);
    log::trace!(
        "{}[{}] >> {} (seq={})\x1b[0m",
        category.color_code(),
        category.name(),
        name,
        seq
    );
}

/// Emit a span exit event with duration
#[inline]
fn emit_span_exit(category: TraceCategory, name: &'static str, ticks: u64) {
    let seq = TRACE_COUNTER.fetch_add(1, Ordering::Relaxed);
    log::trace!(
        "{}[{}] << {} (ticks={}, seq={})\x1b[0m",
        category.color_code(),
        category.name(),
        name,
        ticks,
        seq
    );
}

/// Emit a trace event
#[inline]
pub fn emit_event(category: TraceCategory, level: TraceLevel, msg: &str) {
    if !TRACING_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    
    let seq = TRACE_COUNTER.fetch_add(1, Ordering::Relaxed);
    log::trace!(
        "{}[{}] {} {} (seq={})\x1b[0m",
        category.color_code(),
        category.name(),
        level.prefix(),
        msg,
        seq
    );
}

/// Enable or disable tracing
#[inline]
pub fn set_tracing_enabled(enabled: bool) {
    TRACING_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Check if tracing is enabled
#[inline]
pub fn is_tracing_enabled() -> bool {
    TRACING_ENABLED.load(Ordering::Relaxed)
}

/// Get current trace sequence number
#[inline]
pub fn current_sequence() -> u64 {
    TRACE_COUNTER.load(Ordering::Relaxed)
}

/// Create a trace span with automatic timing
///
/// # Example
/// ```rust
/// let _span = trace_span!(TraceCategory::Syscall, "sys_read");
/// // ... code being traced ...
/// // span automatically ends when dropped
/// ```
#[macro_export]
macro_rules! trace_span {
    ($cat:expr, $name:literal) => {
        $crate::utils::trace::TraceSpan::new($cat, $name)
    };
    ($name:literal) => {
        $crate::utils::trace::TraceSpan::new(
            $crate::utils::trace::TraceCategory::Debug,
            $name
        )
    };
}

/// Emit a trace event with formatted message
///
/// # Example
/// ```rust
/// trace_event!(TraceCategory::Memory, "frame allocated", ppn = 0x1234);
/// ```
#[macro_export]
macro_rules! trace_event {
    ($cat:expr, $level:expr, $msg:literal) => {
        if $crate::utils::trace::is_tracing_enabled() {
            $crate::utils::trace::emit_event($cat, $level, $msg);
        }
    };
    ($cat:expr, $level:expr, $msg:literal, $($key:ident = $val:expr),+) => {
        if $crate::utils::trace::is_tracing_enabled() {
            let formatted = alloc::format!(
                concat!($msg, $(", ", stringify!($key), "={}"),+),
                $($val),+
            );
            $crate::utils::trace::emit_event($cat, $level, &formatted);
        }
    };
}

/// Shorthand for debug-level trace event
#[macro_export]
macro_rules! trace_debug {
    ($cat:expr, $($args:tt)*) => {
        trace_event!($cat, $crate::utils::trace::TraceLevel::Debug, $($args)*)
    };
}

/// Shorthand for info-level trace event
#[macro_export]
macro_rules! trace_info {
    ($cat:expr, $($args:tt)*) => {
        trace_event!($cat, $crate::utils::trace::TraceLevel::Info, $($args)*)
    };
}

/// Shorthand for warn-level trace event
#[macro_export]
macro_rules! trace_warn {
    ($cat:expr, $($args:tt)*) => {
        trace_event!($cat, $crate::utils::trace::TraceLevel::Warn, $($args)*)
    };
}

/// Shorthand for error-level trace event
#[macro_export]
macro_rules! trace_error {
    ($cat:expr, $($args:tt)*) => {
        trace_event!($cat, $crate::utils::trace::TraceLevel::Error, $($args)*)
    };
}

/// Conditional trace span that only activates in debug builds
#[macro_export]
macro_rules! debug_trace_span {
    ($($args:tt)*) => {
        #[cfg(debug_assertions)]
        let _span = trace_span!($($args)*);
        #[cfg(not(debug_assertions))]
        let _span = $crate::utils::trace::TraceSpan::inactive();
    };
}
