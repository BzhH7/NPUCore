//! Device drivers
//!
//! This module provides device driver implementations:
//! - Block device drivers (disk, memory block device)
//! - Serial port drivers (NS16550A UART)

pub mod block;
pub mod serial;

pub use block::BLOCK_DEVICE;
#[cfg(feature = "loongarch64")]
pub use serial::ns16550a::Ns16550a;
