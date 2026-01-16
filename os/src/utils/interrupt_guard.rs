//! Interrupt state RAII guard
//!
//! Provides automatic interrupt state management using RAII pattern
//! to prevent forgetting to restore interrupt state.

use crate::hal::{disable_interrupts, restore_interrupts};

/// RAII guard for interrupt state management
///
/// Automatically saves and restores interrupt state.
/// Ensures interrupts are properly restored even if a panic occurs.
///
/// # Example
/// ```
/// {
///     let _guard = InterruptGuard::new();
///     // Interrupts are disabled here
///     // Critical section code
/// } // Interrupts automatically restored here
/// ```
pub struct InterruptGuard {
    /// Previous interrupt state (true if enabled)
    old_state: bool,
}

impl InterruptGuard {
    /// Create a new interrupt guard
    ///
    /// Disables interrupts and saves the previous state
    #[inline]
    pub fn new() -> Self {
        Self {
            old_state: disable_interrupts(),
        }
    }

    /// Get the previous interrupt state
    #[inline]
    pub fn old_state(&self) -> bool {
        self.old_state
    }
}

impl Drop for InterruptGuard {
    /// Restore interrupt state when guard is dropped
    #[inline]
    fn drop(&mut self) {
        restore_interrupts(self.old_state);
    }
}

/// Helper macro for critical sections with interrupt disabled
///
/// # Example
/// ```
/// with_interrupts_disabled!({
///     // Critical section code
///     modify_shared_data();
/// });
/// ```
#[macro_export]
macro_rules! with_interrupts_disabled {
    ($body:block) => {{
        let _guard = $crate::utils::InterruptGuard::new();
        $body
    }};
}
