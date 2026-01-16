//! Extended Kernel Telemetry and Diagnostics System
//!
//! This module provides a comprehensive observability framework for the kernel,
//! including metrics collection, performance counters, and runtime diagnostics.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                      Kernel Telemetry System                            │
//! ├─────────────────────────────────────────────────────────────────────────┤
//! │                                                                         │
//! │  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐               │
//! │  │   Metrics     │  │   Counters    │  │   Histograms  │               │
//! │  │   Registry    │  │   (Per-CPU)   │  │   (Latency)   │               │
//! │  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘               │
//! │          │                  │                  │                        │
//! │          └──────────────────┼──────────────────┘                        │
//! │                             │                                           │
//! │                    ┌────────▼────────┐                                  │
//! │                    │   Aggregator    │                                  │
//! │                    └────────┬────────┘                                  │
//! │                             │                                           │
//! │                    ┌────────▼────────┐                                  │
//! │                    │    Exporters    │                                  │
//! │                    │  (Log/Console)  │                                  │
//! │                    └─────────────────┘                                  │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Components
//!
//! - **MetricsRegistry**: Central registry for all metrics
//! - **Counter**: Monotonically increasing value
//! - **Gauge**: Value that can go up or down
//! - **Histogram**: Distribution of values with percentiles
//!
//! # Example Usage
//!
//! ```rust
//! use crate::utils::telemetry::*;
//!
//! // Register a counter
//! static SYSCALL_COUNT: Counter = Counter::new("syscall_total", "Total syscalls");
//!
//! // Increment on each syscall
//! SYSCALL_COUNT.inc();
//!
//! // Record latency
//! let _timer = SYSCALL_LATENCY.start_timer();
//! // ... work ...
//! // timer records duration on drop
//! ```

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

// ============================================================================
// Metric Types
// ============================================================================

/// A monotonically increasing counter
///
/// Use for tracking cumulative values like total syscalls, bytes transferred, etc.
pub struct Counter {
    /// Counter name for identification
    name: &'static str,
    /// Description/help text
    description: &'static str,
    /// The actual value
    value: AtomicU64,
}

impl Counter {
    /// Create a new counter with name and description
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            value: AtomicU64::new(0),
        }
    }

    /// Increment by 1
    #[inline]
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment by arbitrary amount
    #[inline]
    pub fn add(&self, delta: u64) {
        self.value.fetch_add(delta, Ordering::Relaxed);
    }

    /// Get current value
    #[inline]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Reset to zero
    #[inline]
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }

    /// Get metric name
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Get metric description
    #[inline]
    pub const fn description(&self) -> &'static str {
        self.description
    }
}

/// A gauge that can increase or decrease
///
/// Use for tracking current state like active connections, memory usage, etc.
pub struct Gauge {
    name: &'static str,
    description: &'static str,
    value: AtomicU64,
}

impl Gauge {
    /// Create a new gauge
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            value: AtomicU64::new(0),
        }
    }

    /// Set to specific value
    #[inline]
    pub fn set(&self, val: u64) {
        self.value.store(val, Ordering::Relaxed);
    }

    /// Increment by 1
    #[inline]
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement by 1
    #[inline]
    pub fn dec(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current value
    #[inline]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Get name
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

// ============================================================================
// Histogram for Latency Tracking
// ============================================================================

/// Bucket boundaries for histogram (in nanoseconds)
const LATENCY_BUCKETS: [u64; 16] = [
    100,          // 100ns
    500,          // 500ns
    1_000,        // 1us
    5_000,        // 5us
    10_000,       // 10us
    50_000,       // 50us
    100_000,      // 100us
    500_000,      // 500us
    1_000_000,    // 1ms
    5_000_000,    // 5ms
    10_000_000,   // 10ms
    50_000_000,   // 50ms
    100_000_000,  // 100ms
    500_000_000,  // 500ms
    1_000_000_000,// 1s
    u64::MAX,     // infinity
];

/// Histogram for tracking value distributions
///
/// Particularly useful for latency measurements where you want percentiles.
pub struct Histogram {
    name: &'static str,
    description: &'static str,
    /// Bucket counts
    buckets: [AtomicU64; 16],
    /// Sum of all observed values
    sum: AtomicU64,
    /// Count of observations
    count: AtomicU64,
    /// Minimum observed value
    min: AtomicU64,
    /// Maximum observed value
    max: AtomicU64,
}

impl Histogram {
    /// Create a new histogram
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            buckets: [
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
                AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0),
            ],
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
            min: AtomicU64::new(u64::MAX),
            max: AtomicU64::new(0),
        }
    }

    /// Record a value in the histogram
    #[inline]
    pub fn observe(&self, value: u64) {
        // Find bucket
        let bucket_idx = LATENCY_BUCKETS
            .iter()
            .position(|&b| value <= b)
            .unwrap_or(15);
        
        self.buckets[bucket_idx].fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(value, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
        
        // Update min/max (approximate due to race conditions, but good enough)
        let current_min = self.min.load(Ordering::Relaxed);
        if value < current_min {
            let _ = self.min.compare_exchange(
                current_min, value, Ordering::Relaxed, Ordering::Relaxed
            );
        }
        
        let current_max = self.max.load(Ordering::Relaxed);
        if value > current_max {
            let _ = self.max.compare_exchange(
                current_max, value, Ordering::Relaxed, Ordering::Relaxed
            );
        }
    }

    /// Create a timer that records duration on drop
    pub fn start_timer(&self) -> HistogramTimer<'_> {
        HistogramTimer {
            histogram: self,
            start: crate::timer::get_time_ns() as u64,
        }
    }

    /// Get summary statistics
    pub fn summary(&self) -> HistogramSummary {
        let count = self.count.load(Ordering::Relaxed);
        let sum = self.sum.load(Ordering::Relaxed);
        
        HistogramSummary {
            count,
            sum,
            avg: if count > 0 { sum / count } else { 0 },
            min: self.min.load(Ordering::Relaxed),
            max: self.max.load(Ordering::Relaxed),
        }
    }

    /// Get percentile value (approximate)
    pub fn percentile(&self, p: f64) -> u64 {
        let total = self.count.load(Ordering::Relaxed);
        if total == 0 {
            return 0;
        }

        let target = ((total as f64) * p / 100.0) as u64;
        let mut cumulative = 0u64;

        for (i, bucket) in self.buckets.iter().enumerate() {
            cumulative += bucket.load(Ordering::Relaxed);
            if cumulative >= target {
                return LATENCY_BUCKETS[i];
            }
        }

        LATENCY_BUCKETS[15]
    }

    /// Get name
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

/// Timer guard that records duration to histogram on drop
pub struct HistogramTimer<'a> {
    histogram: &'a Histogram,
    start: u64,
}

impl Drop for HistogramTimer<'_> {
    fn drop(&mut self) {
        let elapsed = (crate::timer::get_time_ns() as u64).saturating_sub(self.start);
        self.histogram.observe(elapsed);
    }
}

/// Summary of histogram statistics
#[derive(Debug, Clone, Copy)]
pub struct HistogramSummary {
    pub count: u64,
    pub sum: u64,
    pub avg: u64,
    pub min: u64,
    pub max: u64,
}

// ============================================================================
// Per-CPU Counters
// ============================================================================

use crate::config::MAX_CPU_NUM;

/// Per-CPU counter to avoid cache contention
pub struct PerCpuCounter {
    name: &'static str,
    counters: [AtomicU64; MAX_CPU_NUM],
}

impl PerCpuCounter {
    /// Create new per-CPU counter
    pub const fn new(name: &'static str) -> Self {
        const ZERO: AtomicU64 = AtomicU64::new(0);
        Self {
            name,
            counters: [ZERO; MAX_CPU_NUM],
        }
    }

    /// Increment counter for current CPU
    #[inline]
    pub fn inc(&self) {
        let cpu_id = crate::task::processor::current_cpu_id();
        if cpu_id < MAX_CPU_NUM {
            self.counters[cpu_id].fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get sum across all CPUs
    pub fn sum(&self) -> u64 {
        self.counters.iter().map(|c| c.load(Ordering::Relaxed)).sum()
    }

    /// Get per-CPU values
    pub fn per_cpu(&self) -> [u64; MAX_CPU_NUM] {
        let mut result = [0u64; MAX_CPU_NUM];
        for (i, counter) in self.counters.iter().enumerate() {
            result[i] = counter.load(Ordering::Relaxed);
        }
        result
    }
}

// ============================================================================
// Kernel Metrics Instances
// ============================================================================

/// Total syscall count
pub static SYSCALL_TOTAL: Counter = Counter::new(
    "kernel_syscall_total",
    "Total number of system calls executed"
);

/// Active task count
pub static ACTIVE_TASKS: Gauge = Gauge::new(
    "kernel_tasks_active",
    "Number of currently active tasks"
);

/// Syscall latency histogram
pub static SYSCALL_LATENCY: Histogram = Histogram::new(
    "kernel_syscall_latency_ns",
    "System call latency in nanoseconds"
);

/// Page fault count
pub static PAGE_FAULTS: PerCpuCounter = PerCpuCounter::new("kernel_page_faults_total");

/// Context switch count  
pub static CONTEXT_SWITCHES: PerCpuCounter = PerCpuCounter::new("kernel_context_switches_total");

/// Interrupt count
pub static INTERRUPTS: PerCpuCounter = PerCpuCounter::new("kernel_interrupts_total");

// ============================================================================
// Diagnostic Subsystem
// ============================================================================

/// Kernel health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Everything is working normally
    Healthy,
    /// Some non-critical issues detected
    Degraded,
    /// Critical issues that may affect functionality
    Unhealthy,
}

impl HealthStatus {
    /// Convert to string
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
        }
    }
}

/// Diagnostic check result
#[derive(Debug, Clone)]
pub struct DiagnosticResult {
    pub name: &'static str,
    pub status: HealthStatus,
    pub message: Option<String>,
}

impl DiagnosticResult {
    /// Create a healthy result
    pub const fn healthy(name: &'static str) -> Self {
        Self {
            name,
            status: HealthStatus::Healthy,
            message: None,
        }
    }

    /// Create a degraded result
    pub fn degraded(name: &'static str, msg: &str) -> Self {
        Self {
            name,
            status: HealthStatus::Degraded,
            message: Some(String::from(msg)),
        }
    }

    /// Create an unhealthy result
    pub fn unhealthy(name: &'static str, msg: &str) -> Self {
        Self {
            name,
            status: HealthStatus::Unhealthy,
            message: Some(String::from(msg)),
        }
    }
}

/// Run all diagnostic checks
pub fn run_diagnostics() -> Vec<DiagnosticResult> {
    let mut results = Vec::with_capacity(8);

    // Memory check
    results.push(check_memory());
    
    // Task count check
    results.push(check_task_count());
    
    // File descriptor check
    results.push(check_fd_usage());

    results
}

/// Check memory subsystem health
fn check_memory() -> DiagnosticResult {
    let free_frames = crate::mm::unallocated_frames();
    let threshold = 100; // Minimum free frames
    
    if free_frames > threshold * 10 {
        DiagnosticResult::healthy("memory")
    } else if free_frames > threshold {
        DiagnosticResult::degraded("memory", "Low memory warning")
    } else {
        DiagnosticResult::unhealthy("memory", "Critical memory shortage")
    }
}

/// Check task count is reasonable
fn check_task_count() -> DiagnosticResult {
    let active = ACTIVE_TASKS.get();
    let limit = crate::config::SYSTEM_TASK_LIMIT as u64;
    
    if active < limit / 2 {
        DiagnosticResult::healthy("tasks")
    } else if active < limit * 9 / 10 {
        DiagnosticResult::degraded("tasks", "High task count")
    } else {
        DiagnosticResult::unhealthy("tasks", "Task limit nearly reached")
    }
}

/// Check file descriptor usage
fn check_fd_usage() -> DiagnosticResult {
    // Simplified check - in a real implementation, would track global FD usage
    DiagnosticResult::healthy("file_descriptors")
}

// ============================================================================
// Metrics Export
// ============================================================================

/// Format all metrics as a string for logging/display
pub fn format_metrics() -> String {
    use alloc::fmt::Write;
    let mut output = String::with_capacity(1024);

    writeln!(output, "=== Kernel Metrics ===").ok();
    writeln!(output, "{}: {}", SYSCALL_TOTAL.name(), SYSCALL_TOTAL.get()).ok();
    writeln!(output, "{}: {}", ACTIVE_TASKS.name(), ACTIVE_TASKS.get()).ok();
    
    let latency = SYSCALL_LATENCY.summary();
    writeln!(output, "syscall_latency_avg_ns: {}", latency.avg).ok();
    writeln!(output, "syscall_latency_p99_ns: {}", SYSCALL_LATENCY.percentile(99.0)).ok();
    
    writeln!(output, "page_faults_total: {}", PAGE_FAULTS.sum()).ok();
    writeln!(output, "context_switches_total: {}", CONTEXT_SWITCHES.sum()).ok();
    writeln!(output, "interrupts_total: {}", INTERRUPTS.sum()).ok();

    output
}

/// Print metrics to kernel log
pub fn log_metrics() {
    log::info!("{}", format_metrics());
}
