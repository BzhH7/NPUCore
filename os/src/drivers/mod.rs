pub mod block;
pub mod serial;

pub use block::BLOCK_DEVICE;
#[cfg(feature = "loongarch64")]
pub use serial::ns16550a::Ns16550a;
