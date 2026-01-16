use super::{BlockDevice, BLOCK_SZ};
use crate::mm::{
    frame_alloc, frame_dealloc, kernel_token, FrameTracker, PageTable, PageTableImpl, PhysAddr, PhysPageNum,
    StepByOne, VirtAddr,
};
use alloc::{sync::Arc, vec::Vec};
use lazy_static::*;
use spin::Mutex;
use core::ptr::NonNull;
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::mmio::{MmioTransport, VirtIOHeader};
use virtio_drivers::{Hal, BufferDirection};

/// VirtIO block device sector size (512 bytes)
const VIRTIO_BLK_SIZE: usize = 512;

#[allow(unused)]
const VIRTIO0: usize = 0x10001000;

pub struct VirtIOBlock(Mutex<VirtIOBlk<VirtioHal, MmioTransport>>);

lazy_static! {
    static ref QUEUE_FRAMES: Mutex<Vec<Arc<FrameTracker>>> = Mutex::new(Vec::new());
}

impl BlockDevice for VirtIOBlock {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        // Convert filesystem block to virtio sectors
        let sectors_per_block = BLOCK_SZ / VIRTIO_BLK_SIZE;
        let start_sector = block_id * sectors_per_block;
        for (i, chunk) in buf.chunks_mut(VIRTIO_BLK_SIZE).enumerate() {
            self.0
                .lock()
                .read_blocks(start_sector, chunk)
                .expect("Error when reading VirtIOBlk");
        }
    }
    fn write_block(&self, block_id: usize, buf: &[u8]) {
        // Convert filesystem block to virtio sectors
        let sectors_per_block = BLOCK_SZ / VIRTIO_BLK_SIZE;
        let start_sector = block_id * sectors_per_block;
        for (i, chunk) in buf.chunks(VIRTIO_BLK_SIZE).enumerate() {
            self.0
                .lock()
                .write_blocks(start_sector, chunk)
                .expect("Error when writing VirtIOBlk");
        }
    }
}

impl VirtIOBlock {
    #[allow(unused)]
    pub fn new() -> Self {
        let header = NonNull::new(VIRTIO0 as *mut VirtIOHeader).unwrap();
        let transport = unsafe { MmioTransport::new(header) }.unwrap();
        Self(Mutex::new(
            VirtIOBlk::<VirtioHal, MmioTransport>::new(transport).unwrap(),
        ))
    }
}

pub struct VirtioHal;

unsafe impl Hal for VirtioHal {
    fn dma_alloc(pages: usize, _dir: BufferDirection) -> (usize, NonNull<u8>) {
        let paddr = virtio_dma_alloc(pages);
        let vaddr = virtio_phys_to_virt(PhysAddr(paddr));
        (paddr, NonNull::new(vaddr.0 as *mut u8).unwrap())
    }

    unsafe fn dma_dealloc(paddr: usize, _vaddr: NonNull<u8>, pages: usize) -> i32 {
        virtio_dma_dealloc(PhysAddr(paddr), pages)
    }

    unsafe fn mmio_phys_to_virt(paddr: usize, _size: usize) -> NonNull<u8> {
        NonNull::new(paddr as *mut u8).unwrap()
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> usize {
        let vaddr = buffer.as_ptr() as *mut u8 as usize;
        virtio_virt_to_phys(VirtAddr(vaddr)).0
    }

    unsafe fn unshare(_paddr: usize, _buffer: NonNull<[u8]>, _direction: BufferDirection) {
        // No-op for identity mapping
    }
}

fn virtio_dma_alloc(pages: usize) -> usize {
    let mut ppn_base = PhysPageNum(0);
    for i in 0..pages {
        let frame = frame_alloc().unwrap();
        if i == 0 {
            ppn_base = frame.ppn;
        }
        assert_eq!(frame.ppn.0, ppn_base.0 + i);
        QUEUE_FRAMES.lock().push(frame);
    }
    let addr: PhysAddr = ppn_base.into();
    addr.0
}

fn virtio_dma_dealloc(pa: PhysAddr, pages: usize) -> i32 {
    let mut ppn_base: PhysPageNum = pa.into();
    for _ in 0..pages {
        frame_dealloc(ppn_base);
        ppn_base.step();
    }
    0
}

fn virtio_phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    VirtAddr(paddr.0)
}

lazy_static! {
    static ref KERNEL_TOKEN: usize = kernel_token();
}

fn virtio_virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
    PageTableImpl::from_token(*KERNEL_TOKEN)
        .translate_va(vaddr)
        .unwrap()
}
