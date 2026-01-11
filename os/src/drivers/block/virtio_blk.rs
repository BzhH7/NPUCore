use super::{BlockDevice, BLOCK_SZ};
use crate::mm::{
    frame_alloc, frame_dealloc, kernel_token, FrameTracker, PageTable, PageTableImpl, PhysAddr, PhysPageNum,
    StepByOne, VirtAddr,
};
use alloc::{sync::Arc, vec::Vec};
use lazy_static::*;
use spin::Mutex;
use virtio_drivers::{VirtIOBlk, VirtIOHeader};

/// VirtIO block device sector size (512 bytes)
const VIRTIO_BLK_SIZE: usize = 512;

#[allow(unused)]
const VIRTIO0: usize = 0x10001000;

pub struct VirtIOBlock(Mutex<VirtIOBlk<'static>>);

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
                .read_block(start_sector + i, chunk)
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
                .write_block(start_sector + i, chunk)
                .expect("Error when writing VirtIOBlk");
        }
    }
}

impl VirtIOBlock {
    #[allow(unused)]
    pub fn new() -> Self {
        Self(Mutex::new(
            VirtIOBlk::new(unsafe { &mut *(VIRTIO0 as *mut VirtIOHeader) }).unwrap(),
        ))
    }
}

#[no_mangle]
pub extern "C" fn virtio_dma_alloc(pages: usize) -> PhysAddr {
    let mut ppn_base = PhysPageNum(0);
    for i in 0..pages {
        let frame = frame_alloc().unwrap();
        if i == 0 {
            ppn_base = frame.ppn;
        }
        assert_eq!(frame.ppn.0, ppn_base.0 + i);
        QUEUE_FRAMES.lock().push(frame);
    }
    ppn_base.into()
}

#[no_mangle]
pub extern "C" fn virtio_dma_dealloc(pa: PhysAddr, pages: usize) -> i32 {
    let mut ppn_base: PhysPageNum = pa.into();
    for _ in 0..pages {
        frame_dealloc(ppn_base);
        ppn_base.step();
    }
    0
}

#[no_mangle]
pub extern "C" fn virtio_phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    VirtAddr(paddr.0)
}

lazy_static! {
    static ref KERNEL_TOKEN: usize = kernel_token();
}

#[no_mangle]
pub extern "C" fn virtio_virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
    PageTableImpl::from_token(*KERNEL_TOKEN)
        .translate_va(vaddr)
        .unwrap()
}
