//! Block device trait definition
//!
//! Defines the common interface for all block storage devices

use core::any::Any;

use crate::hal::BLOCK_SZ;

/// Block device trait
///
/// Provides block-level read/write operations for storage devices.
/// All operations work with BLOCK_SZ-sized blocks.
///
/// # Notes on error handling
/// - What if buf.len() > BLOCK_SZ for read_block?
/// - Does read_block zero the rest when buf.len() != BLOCK_SZ?
/// - What if buf.len() < BLOCK_SZ for write_block?
pub trait BlockDevice: Send + Sync + Any {
    /// Read a block from the device
    ///
    /// # Arguments
    /// * `block_id` - Block number to read
    /// * `buf` - Buffer to store read data
    ///
    /// # Panics
    /// May panic if buf size is not a multiple of BLOCK_SZ (implementation-dependent)
    fn read_block(&self, block_id: usize, buf: &mut [u8]);

    /// Write a block to the device
    ///
    /// # Arguments
    /// * `block_id` - Block number to write
    /// * `buf` - Buffer containing data to write
    ///
    /// # Panics
    /// May panic if buf size is not a multiple of BLOCK_SZ (implementation-dependent)
    fn write_block(&self, block_id: usize, buf: &[u8]);

    /// Clear a block (fill with specified byte value)
    ///
    /// # Arguments
    /// * `block_id` - Block number to clear
    /// * `num` - Byte value to fill with
    ///
    /// # Note
    /// K210 supports native multi-block clear which could optimize this
    fn clear_block(&self, block_id: usize, num: u8) {
        self.write_block(block_id, &[num; BLOCK_SZ]);
    }

    /// Clear multiple consecutive blocks
    ///
    /// # Arguments
    /// * `block_id` - Starting block number
    /// * `cnt` - Number of blocks to clear
    /// * `num` - Byte value to fill with
    ///
    /// # Note
    /// K210 supports native multi-block clear which could optimize this
    fn clear_mult_block(&self, block_id: usize, cnt: usize, num: u8) {
        for i in block_id..block_id + cnt {
            self.write_block(i, &[num; BLOCK_SZ]);
        }
    }
}
