//! Task context for context switching
//!
//! Defines the TaskContext structure that stores the minimal CPU state
//! needed for task switching, including return address, stack pointer,
//! and callee-saved registers.

use crate::hal::trap_return;

/// Task context for context switching
///
/// Contains the minimal CPU state needed to resume task execution:
/// - Return address (ra): where to jump when switching to this task
/// - Stack pointer (sp): kernel stack pointer
/// - Saved registers (s0-s11): callee-saved registers
#[repr(C)]
pub struct TaskContext {
    /// Return address
    ra: usize,
    /// Stack pointer
    sp: usize,
    /// Callee-saved registers (s0-s11)
    s: [usize; 12],
}

impl TaskContext {
    /// Create a zero-initialized task context
    pub fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    /// Create a task context that returns to trap handler
    ///
    /// # Arguments
    /// * `kstack_ptr` - Kernel stack pointer
    pub fn goto_trap_return(kstack_ptr: usize) -> Self {
        Self {
            ra: trap_return as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}
