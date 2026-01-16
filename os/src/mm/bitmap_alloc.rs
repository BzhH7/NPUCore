//! Bitmap-based frame allocator
//!
//! This module provides an alternative frame allocation strategy using a bitmap
//! instead of the traditional stack-based approach. Each bit represents a physical
//! frame: 0 = free, 1 = allocated.
//!
//! # Advantages over Stack Allocator
//!
//! 1. **Constant memory overhead**: O(N/8) bytes for N frames vs O(N) for stack
//! 2. **Contiguous allocation support**: Easy to find consecutive free frames
//! 3. **Better cache behavior**: Sequential scanning is cache-friendly
//! 4. **Fast bulk operations**: Can clear/set multiple frames atomically
//!
//! # Implementation Details
//!
//! The bitmap is stored as an array of `u64` words, each tracking 64 frames.
//! A hint pointer tracks the last allocation position for faster subsequent
//! allocations (next-fit strategy).

use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Bitmap-based physical frame allocator
///
/// Uses a compact bitmap representation where each bit represents one physical frame.
/// Implements a next-fit allocation strategy for improved temporal locality.
pub struct BitmapFrameAllocator {
    /// Bitmap storage: each u64 tracks 64 frames
    bitmap: Vec<u64>,
    /// Start physical page number
    start_ppn: usize,
    /// Total number of frames managed
    total_frames: usize,
    /// Hint for next allocation (word index)
    alloc_hint: usize,
    /// Count of allocated frames
    allocated_count: usize,
}

impl BitmapFrameAllocator {
    /// Bits per bitmap word
    const BITS_PER_WORD: usize = 64;
    
    /// Create a new uninitialized bitmap allocator
    pub const fn new() -> Self {
        Self {
            bitmap: Vec::new(),
            start_ppn: 0,
            total_frames: 0,
            alloc_hint: 0,
            allocated_count: 0,
        }
    }
    
    /// Initialize the allocator with a physical page range
    ///
    /// # Arguments
    /// * `start` - First physical page number
    /// * `end` - One past the last physical page number
    pub fn init(&mut self, start: usize, end: usize) {
        assert!(start < end, "Invalid frame range");
        
        self.start_ppn = start;
        self.total_frames = end - start;
        self.alloc_hint = 0;
        self.allocated_count = 0;
        
        // Calculate bitmap size (round up to nearest word)
        let word_count = (self.total_frames + Self::BITS_PER_WORD - 1) / Self::BITS_PER_WORD;
        
        // Initialize bitmap: all zeros = all frames free
        self.bitmap = vec![0u64; word_count];
        
        // Mark any padding bits at the end as allocated (to prevent invalid allocation)
        let valid_bits_in_last = self.total_frames % Self::BITS_PER_WORD;
        if valid_bits_in_last != 0 && !self.bitmap.is_empty() {
            let last_idx = self.bitmap.len() - 1;
            // Set all bits beyond valid range to 1 (allocated)
            let mask = !((1u64 << valid_bits_in_last) - 1);
            self.bitmap[last_idx] |= mask;
        }
    }
    
    /// Get the number of unallocated (free) frames
    #[inline]
    pub fn unallocated_frames(&self) -> usize {
        self.total_frames - self.allocated_count
    }
    
    /// Get the number of allocated frames
    #[inline]
    pub fn allocated_frames(&self) -> usize {
        self.allocated_count
    }
    
    /// Get total managed frames
    #[inline]
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }
    
    /// Convert frame index to physical page number
    #[inline]
    fn index_to_ppn(&self, idx: usize) -> usize {
        self.start_ppn + idx
    }
    
    /// Convert physical page number to frame index
    #[inline]
    fn ppn_to_index(&self, ppn: usize) -> Option<usize> {
        if ppn >= self.start_ppn && ppn < self.start_ppn + self.total_frames {
            Some(ppn - self.start_ppn)
        } else {
            None
        }
    }
    
    /// Find and allocate a single free frame using next-fit strategy
    ///
    /// # Returns
    /// * `Some(ppn)` - Physical page number of allocated frame
    /// * `None` - No free frames available
    pub fn alloc_frame(&mut self) -> Option<usize> {
        let word_count = self.bitmap.len();
        if word_count == 0 {
            return None;
        }
        
        // Start searching from hint position
        let start_word = self.alloc_hint;
        
        // Search from hint to end
        for offset in 0..word_count {
            let word_idx = (start_word + offset) % word_count;
            let word = self.bitmap[word_idx];
            
            // Check if any bit is free (0)
            if word != u64::MAX {
                // Find first zero bit using trailing ones count
                let bit_idx = (!word).trailing_zeros() as usize;
                let frame_idx = word_idx * Self::BITS_PER_WORD + bit_idx;
                
                // Verify frame is within valid range
                if frame_idx < self.total_frames {
                    // Mark frame as allocated
                    self.bitmap[word_idx] |= 1u64 << bit_idx;
                    self.allocated_count += 1;
                    
                    // Update hint for next allocation
                    self.alloc_hint = word_idx;
                    
                    return Some(self.index_to_ppn(frame_idx));
                }
            }
        }
        
        None
    }
    
    /// Allocate multiple contiguous frames
    ///
    /// # Arguments
    /// * `count` - Number of contiguous frames needed
    ///
    /// # Returns
    /// * `Some(ppn)` - Starting physical page number of allocated region
    /// * `None` - Not enough contiguous frames available
    pub fn alloc_contiguous(&mut self, count: usize) -> Option<usize> {
        if count == 0 {
            return None;
        }
        if count == 1 {
            return self.alloc_frame();
        }
        
        // Simple linear search for contiguous region
        let mut consecutive = 0;
        let mut start_idx = 0;
        
        for idx in 0..self.total_frames {
            let word_idx = idx / Self::BITS_PER_WORD;
            let bit_idx = idx % Self::BITS_PER_WORD;
            
            let is_free = (self.bitmap[word_idx] & (1u64 << bit_idx)) == 0;
            
            if is_free {
                if consecutive == 0 {
                    start_idx = idx;
                }
                consecutive += 1;
                
                if consecutive >= count {
                    // Found enough contiguous frames - mark them as allocated
                    for i in start_idx..start_idx + count {
                        let w = i / Self::BITS_PER_WORD;
                        let b = i % Self::BITS_PER_WORD;
                        self.bitmap[w] |= 1u64 << b;
                    }
                    self.allocated_count += count;
                    return Some(self.index_to_ppn(start_idx));
                }
            } else {
                consecutive = 0;
            }
        }
        
        None
    }
    
    /// Deallocate a frame
    ///
    /// # Arguments
    /// * `ppn` - Physical page number to free
    ///
    /// # Panics
    /// Panics in debug mode if frame was not allocated
    pub fn dealloc_frame(&mut self, ppn: usize) {
        let Some(frame_idx) = self.ppn_to_index(ppn) else {
            panic!("BitmapAllocator: ppn {:#x} out of range", ppn);
        };
        
        let word_idx = frame_idx / Self::BITS_PER_WORD;
        let bit_idx = frame_idx % Self::BITS_PER_WORD;
        
        // Debug check: verify frame was allocated
        #[cfg(debug_assertions)]
        {
            if (self.bitmap[word_idx] & (1u64 << bit_idx)) == 0 {
                panic!("BitmapAllocator: double free of ppn {:#x}", ppn);
            }
        }
        
        // Clear the bit (mark as free)
        self.bitmap[word_idx] &= !(1u64 << bit_idx);
        self.allocated_count -= 1;
        
        // Update hint to this position (may help next allocation)
        self.alloc_hint = word_idx;
    }
    
    /// Check if a frame is allocated
    #[inline]
    pub fn is_allocated(&self, ppn: usize) -> bool {
        self.ppn_to_index(ppn)
            .map(|idx| {
                let word_idx = idx / Self::BITS_PER_WORD;
                let bit_idx = idx % Self::BITS_PER_WORD;
                (self.bitmap[word_idx] & (1u64 << bit_idx)) != 0
            })
            .unwrap_or(false)
    }
    
    /// Get allocation statistics
    pub fn statistics(&self) -> AllocatorStats {
        AllocatorStats {
            total_frames: self.total_frames,
            allocated_frames: self.allocated_count,
            free_frames: self.unallocated_frames(),
            bitmap_words: self.bitmap.len(),
            bitmap_bytes: self.bitmap.len() * 8,
        }
    }
}

/// Allocation statistics for debugging and monitoring
#[derive(Debug, Clone, Copy)]
pub struct AllocatorStats {
    /// Total managed frames
    pub total_frames: usize,
    /// Currently allocated frames
    pub allocated_frames: usize,
    /// Currently free frames
    pub free_frames: usize,
    /// Number of u64 words in bitmap
    pub bitmap_words: usize,
    /// Bitmap memory usage in bytes
    pub bitmap_bytes: usize,
}

impl core::fmt::Display for AllocatorStats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "FrameAllocator: {}/{} frames used ({:.1}%), bitmap: {} bytes",
            self.allocated_frames,
            self.total_frames,
            (self.allocated_frames as f64 / self.total_frames as f64) * 100.0,
            self.bitmap_bytes
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bitmap_allocator_basic() {
        let mut alloc = BitmapFrameAllocator::new();
        alloc.init(0x1000, 0x1100); // 256 frames
        
        assert_eq!(alloc.total_frames(), 256);
        assert_eq!(alloc.unallocated_frames(), 256);
        
        // Allocate some frames
        let f1 = alloc.alloc_frame().unwrap();
        let f2 = alloc.alloc_frame().unwrap();
        
        assert_eq!(alloc.allocated_frames(), 2);
        assert!(alloc.is_allocated(f1));
        assert!(alloc.is_allocated(f2));
        
        // Deallocate
        alloc.dealloc_frame(f1);
        assert!(!alloc.is_allocated(f1));
        assert_eq!(alloc.allocated_frames(), 1);
    }
    
    #[test]
    fn test_contiguous_allocation() {
        let mut alloc = BitmapFrameAllocator::new();
        alloc.init(0, 1024);
        
        // Allocate 10 contiguous frames
        let start = alloc.alloc_contiguous(10).unwrap();
        assert_eq!(alloc.allocated_frames(), 10);
        
        // All 10 should be allocated
        for i in 0..10 {
            assert!(alloc.is_allocated(start + i));
        }
    }
}
