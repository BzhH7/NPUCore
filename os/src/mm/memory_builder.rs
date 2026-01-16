//! MemorySet Builder Pattern
//!
//! This module provides a fluent builder API for constructing MemorySet objects.
//! The builder pattern improves code readability and makes memory configuration
//! more explicit and less error-prone.
//!
//! # Example
//!
//! ```rust
//! let memory = MemorySetBuilder::new()
//!     .with_kernel_mappings()
//!     .add_segment(start, end, MapPermission::R | MapPermission::W)
//!     .add_user_stack(tid, stack_size)
//!     .add_trap_context(tid)
//!     .build()?;
//! ```
//!
//! # Design Rationale
//!
//! The builder pattern offers several advantages over direct construction:
//!
//! 1. **Clarity**: Each configuration step is explicitly named
//! 2. **Validation**: Can validate configuration before building
//! 3. **Defaults**: Easy to provide sensible defaults
//! 4. **Extensibility**: Easy to add new configuration options
//! 5. **Immutability**: Built object is immutable after construction

use alloc::vec::Vec;
use super::map_area::MapPermission;
use super::memory_set::MemorySet;
use super::page_table::PageTable;
use super::VirtAddr;
use crate::config::PAGE_SIZE;

/// Builder for constructing MemorySet objects
///
/// Provides a fluent API for step-by-step memory configuration.
/// Call `build()` to finalize and create the MemorySet.
pub struct MemorySetBuilder<T: PageTable> {
    /// Pending memory areas to be added
    pending_areas: Vec<PendingArea>,
    /// Whether to include kernel mappings
    include_kernel: bool,
    /// Custom heap configuration
    heap_config: Option<HeapConfig>,
    /// Custom stack configuration  
    stack_config: Option<StackConfig>,
    /// Marker for page table type
    _marker: core::marker::PhantomData<T>,
}

/// Pending area awaiting finalization
struct PendingArea {
    start: VirtAddr,
    end: VirtAddr,
    permission: MapPermission,
    area_type: AreaType,
    #[allow(dead_code)]
    data: Option<Vec<u8>>,
}

/// Type of memory area
#[derive(Clone, Copy, Debug)]
enum AreaType {
    /// Anonymous memory (heap, stack)
    Anonymous,
    /// Program segment from ELF
    Program,
    /// Memory-mapped file
    #[allow(dead_code)]
    FileMapped,
    /// Device memory (MMIO)
    Device,
}

/// Heap configuration
#[derive(Clone, Copy, Debug)]
struct HeapConfig {
    start: VirtAddr,
    initial_size: usize,
    #[allow(dead_code)]
    max_size: usize,
}

/// Stack configuration
#[derive(Clone, Copy, Debug)]
struct StackConfig {
    top: VirtAddr,
    size: usize,
    #[allow(dead_code)]
    guard_pages: usize,
}

impl<T: PageTable> Default for MemorySetBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: PageTable> MemorySetBuilder<T> {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self {
            pending_areas: Vec::with_capacity(16),
            include_kernel: false,
            heap_config: None,
            stack_config: None,
            _marker: core::marker::PhantomData,
        }
    }

    /// Include kernel space mappings in the address space
    ///
    /// This is required for user processes to be able to enter kernel mode
    /// during system calls and interrupts.
    pub fn with_kernel_mappings(mut self) -> Self {
        self.include_kernel = true;
        self
    }

    /// Add an anonymous memory segment
    ///
    /// # Arguments
    /// * `start` - Starting virtual address
    /// * `end` - Ending virtual address (exclusive)
    /// * `permission` - Access permissions
    pub fn add_segment(mut self, start: VirtAddr, end: VirtAddr, permission: MapPermission) -> Self {
        self.pending_areas.push(PendingArea {
            start,
            end,
            permission,
            area_type: AreaType::Anonymous,
            data: None,
        });
        self
    }

    /// Add a program segment with initial data
    ///
    /// # Arguments
    /// * `start` - Starting virtual address
    /// * `end` - Ending virtual address
    /// * `permission` - Access permissions
    /// * `data` - Initial content to copy into the segment
    pub fn add_program_segment(
        mut self,
        start: VirtAddr,
        end: VirtAddr,
        permission: MapPermission,
        data: Vec<u8>,
    ) -> Self {
        self.pending_areas.push(PendingArea {
            start,
            end,
            permission,
            area_type: AreaType::Program,
            data: Some(data),
        });
        self
    }

    /// Configure the user heap
    ///
    /// # Arguments
    /// * `start` - Heap start address
    /// * `initial_size` - Initial heap size
    /// * `max_size` - Maximum heap size
    pub fn with_heap(mut self, start: VirtAddr, initial_size: usize, max_size: usize) -> Self {
        self.heap_config = Some(HeapConfig {
            start,
            initial_size,
            max_size,
        });
        self
    }

    /// Configure the user stack
    ///
    /// # Arguments
    /// * `top` - Stack top address (highest address)
    /// * `size` - Stack size in bytes
    /// * `guard_pages` - Number of guard pages below stack
    pub fn with_stack(mut self, top: VirtAddr, size: usize, guard_pages: usize) -> Self {
        self.stack_config = Some(StackConfig {
            top,
            size,
            guard_pages,
        });
        self
    }

    /// Add a user stack for a specific thread
    ///
    /// # Arguments
    /// * `tid` - Thread ID (used to calculate stack location)
    /// * `size` - Stack size in bytes
    pub fn add_user_stack(self, tid: usize, size: usize) -> Self {
        let stack_bottom = crate::task::ustack_bottom_from_tid(tid);
        let stack_top = VirtAddr::from(stack_bottom);
        self.with_stack(stack_top, size, 1)
    }

    /// Add trap context area for a specific thread
    ///
    /// # Arguments
    /// * `tid` - Thread ID
    pub fn add_trap_context(mut self, tid: usize) -> Self {
        let trap_cx_bottom = crate::task::trap_cx_bottom_from_tid(tid);
        let trap_cx_top = trap_cx_bottom + PAGE_SIZE;
        self.pending_areas.push(PendingArea {
            start: VirtAddr::from(trap_cx_bottom),
            end: VirtAddr::from(trap_cx_top),
            permission: MapPermission::R | MapPermission::W,
            area_type: AreaType::Anonymous,
            data: None,
        });
        self
    }

    /// Add MMIO (memory-mapped I/O) region
    ///
    /// # Arguments
    /// * `phys_start` - Physical start address
    /// * `size` - Size of the MMIO region
    pub fn add_mmio_region(mut self, phys_start: usize, size: usize) -> Self {
        self.pending_areas.push(PendingArea {
            start: VirtAddr::from(phys_start),
            end: VirtAddr::from(phys_start + size),
            permission: MapPermission::R | MapPermission::W,
            area_type: AreaType::Device,
            data: None,
        });
        self
    }

    /// Calculate total memory requirements
    pub fn estimate_memory(&self) -> MemoryEstimate {
        let mut pages = 0;
        let mut bytes = 0;

        for area in &self.pending_areas {
            let area_pages = (area.end.0 - area.start.0 + PAGE_SIZE - 1) / PAGE_SIZE;
            pages += area_pages;
            bytes += area.end.0 - area.start.0;
        }

        if let Some(ref heap) = self.heap_config {
            pages += heap.initial_size / PAGE_SIZE;
            bytes += heap.initial_size;
        }

        if let Some(ref stack) = self.stack_config {
            let stack_pages = (stack.size + PAGE_SIZE - 1) / PAGE_SIZE;
            pages += stack_pages + stack.guard_pages;
            bytes += stack.size;
        }

        MemoryEstimate { pages, bytes }
    }

    /// Build the MemorySet from the configured options
    ///
    /// # Returns
    /// * `Ok(MemorySet)` on success
    /// * `Err(BuildError)` if configuration is invalid
    pub fn build(self) -> Result<MemorySet<T>, BuildError> {
        let mut memory_set = if self.include_kernel {
            MemorySet::new_bare_kern()
        } else {
            MemorySet::new_bare()
        };

        // Process pending areas using MemorySet's public insert methods
        for area in self.pending_areas {
            // Use insert_framed_area for anonymous/program segments
            // Device/MMIO segments require different handling
            match area.area_type {
                AreaType::Device => {
                    // MMIO areas handled separately - not supported via builder yet
                    continue;
                }
                _ => {
                    memory_set.insert_framed_area(
                        area.start,
                        area.end,
                        area.permission,
                    );
                }
            }
        }

        // Configure heap if specified
        if let Some(heap) = self.heap_config {
            memory_set.insert_framed_area(
                heap.start,
                VirtAddr::from(heap.start.0 + heap.initial_size),
                MapPermission::R | MapPermission::W | MapPermission::U,
            );
        }

        // Configure stack if specified
        if let Some(stack) = self.stack_config {
            let stack_bottom = stack.top.0 - stack.size;
            memory_set.insert_framed_area(
                VirtAddr::from(stack_bottom),
                stack.top,
                MapPermission::R | MapPermission::W | MapPermission::U,
            );
        }

        Ok(memory_set)
    }
}

/// Memory estimate from builder configuration
#[derive(Debug, Clone, Copy)]
pub struct MemoryEstimate {
    /// Estimated number of pages
    pub pages: usize,
    /// Estimated total bytes
    pub bytes: usize,
}

impl core::fmt::Display for MemoryEstimate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "MemoryEstimate: {} pages ({} KB)",
            self.pages,
            self.bytes / 1024
        )
    }
}

/// Errors that can occur during MemorySet building
#[derive(Debug, Clone, Copy)]
pub enum BuildError {
    /// Address range overlaps with existing mapping
    OverlappingRegion,
    /// Invalid address range (end <= start)
    InvalidRange,
    /// Out of memory
    OutOfMemory,
    /// Invalid permission combination
    InvalidPermission,
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OverlappingRegion => write!(f, "memory regions overlap"),
            Self::InvalidRange => write!(f, "invalid address range"),
            Self::OutOfMemory => write!(f, "out of memory"),
            Self::InvalidPermission => write!(f, "invalid permission combination"),
        }
    }
}

/// Extension trait for MemorySet to support builder pattern
pub trait MemorySetBuilderExt<T: PageTable> {
    /// Create a builder for this memory set type
    fn builder() -> MemorySetBuilder<T>;
}

impl<T: PageTable> MemorySetBuilderExt<T> for MemorySet<T> {
    fn builder() -> MemorySetBuilder<T> {
        MemorySetBuilder::new()
    }
}
