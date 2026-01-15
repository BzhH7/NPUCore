//! Block device drivers
//!
//! Provides unified block device interface with multiple implementations:
//! - Memory block device (for testing without real storage)
//! - SATA disk driver
//! - VirtIO block device (MMIO and PCI variants)
//!
//! The actual implementation is selected at compile time via feature flags.

mod block_dev;
mod mem_blk;
mod sata_blk;
#[cfg(feature = "block_virt")]
mod virtio_blk;
#[cfg(feature = "block_virt_pci")]
mod virtio_blk_pci;

pub use block_dev::BlockDevice;

// Select block device implementation based on features
#[cfg(feature = "block_mem")]
type BlockDeviceImpl = mem_blk::MemBlockWrapper;
#[cfg(feature = "block_sata")]
type BlockDeviceImpl = sata_blk::SataBlock;
#[cfg(feature = "block_virt")]
type BlockDeviceImpl = virtio_blk::VirtIOBlock;
#[cfg(feature = "block_virt_pci")]
type BlockDeviceImpl = virtio_blk_pci::VirtIOBlock;

use crate::hal::BLOCK_SZ;
use alloc::sync::Arc;
use lazy_static::*;

lazy_static! {
    /// Global block device instance
    pub static ref BLOCK_DEVICE: Arc<dyn BlockDevice> = Arc::new(BlockDeviceImpl::new());
}

/// Test block device read/write operations
///
/// Writes and reads back all blocks to verify correct operation
#[allow(unused)]
pub fn block_device_test() {
    let block_device = BLOCK_DEVICE.clone();
    let mut write_buffer = [0u8; BLOCK_SZ];
    let mut read_buffer = [0u8; BLOCK_SZ];
    for i in 0..BLOCK_SZ {
        for byte in write_buffer.iter_mut() {
            *byte = i as u8;
        }
        block_device.write_block(i as usize, &write_buffer);
        block_device.read_block(i as usize, &mut read_buffer);
        assert_eq!(write_buffer, read_buffer);
    }
    println!("block device test passed!");
}
