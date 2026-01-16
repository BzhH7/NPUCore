//! Process ID (PID) allocation and management
//!
//! This module provides:
//! - PID allocation with recycling
//! - Thread ID allocation
//! - RAII-based PID handle for automatic deallocation

pub use crate::hal::{trap_cx_bottom_from_tid, ustack_bottom_from_tid};
use alloc::vec::Vec;
use lazy_static::*;
use spin::Mutex;

/// ID allocator with recycling support
///
/// Allocates IDs sequentially and recycles deallocated IDs for reuse.
/// Used for both process IDs (PIDs) and thread IDs (TIDs).
pub struct RecycleAllocator {
    /// Next ID to allocate if no recycled IDs available
    current: usize,
    /// Recycled IDs available for reuse
    recycled: Vec<usize>,
}

impl RecycleAllocator {
    /// Create a new allocator starting from ID 1
    pub fn new() -> Self {
        RecycleAllocator {
            current: 1,
            recycled: Vec::new(),
        }
    }

    /// Allocate a new ID
    ///
    /// Returns a recycled ID if available, otherwise allocates a new one
    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.recycled.pop() {
            id
        } else {
            self.current += 1;
            self.current - 1
        }
    }

    /// Deallocate an ID for recycling
    ///
    /// # Arguments
    /// * `id` - ID to deallocate
    ///
    /// # Panics
    /// Panics if ID is out of range or already deallocated
    pub fn dealloc(&mut self, id: usize) {
        assert!(id < self.current);
        assert!(
            !self.recycled.iter().any(|i| *i == id),
            "id {} has been deallocated!",
            id
        );
        self.recycled.push(id);
    }

    /// Get count of currently allocated IDs
    pub fn get_allocated(&self) -> usize {
        self.current - self.recycled.len()
    }
}

lazy_static! {
    /// Global PID allocator
    static ref PID_ALLOCATOR: Mutex<RecycleAllocator> = Mutex::new(RecycleAllocator::new());
}

/// RAII handle for a process ID
///
/// Automatically deallocates the PID when dropped
pub struct PidHandle(pub usize);

/// Allocate a new PID
pub fn pid_alloc() -> PidHandle {
    PidHandle(PID_ALLOCATOR.lock().alloc())
}

impl Drop for PidHandle {
    fn drop(&mut self) {
        PID_ALLOCATOR.lock().dealloc(self.0);
    }
}
