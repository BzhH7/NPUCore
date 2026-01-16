//! Kernel utility modules
//!
//! This module provides common utilities used throughout the kernel:
//! - Error handling framework (`kerror`)
//! - Legacy error types (`error`)
//! - Interrupt management (`interrupt_guard`)
//! - Random number generation (`random`)
//! - Tracing and instrumentation (`trace`)
//! - Telemetry and metrics (`telemetry`)

pub mod error;
pub mod interrupt_guard;
pub mod kerror;
pub mod random;
pub mod telemetry;
pub mod trace;

pub use interrupt_guard::InterruptGuard;
pub use kerror::{KernelError, KernelResult, OptionExt, ResultExt};