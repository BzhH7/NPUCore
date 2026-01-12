use super::{BlockDevice, BLOCK_SZ};
use crate::mm::{
    frame_alloc, frame_dealloc, kernel_token, FrameTracker, PageTable, PageTableImpl, PhysAddr, PhysPageNum,
    StepByOne, VirtAddr,
};
use alloc::{sync::Arc, vec::Vec};
use lazy_static::*;
use spin::Mutex;
use virtio_drivers::{BufferDirection, Hal, PhysAddr as VirtioPhysAddr};
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::mmio::{MmioTransport, VirtIOHeader};
use core::ptr::NonNull;

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
                .read_blocks(start_sector + i, chunk)
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
                .write_blocks(start_sector + i, chunk)
                .expect("Error when writing VirtIOBlk");
        }
    }
}

impl VirtIOBlock {
    #[allow(unused)]
    pub fn new() -> Self {
        let header = NonNull::new(VIRTIO0 as *mut VirtIOHeader).unwrap();
        let transport = unsafe { MmioTransport::new(header) }.unwrap();
        let device = VirtIOBlk::new(transport).unwrap();
        Self(Mutex::new(device))
    }
}

// 定义 VirtioHal 并实现 Hal Trait
pub struct VirtioHal;

unsafe impl Hal for VirtioHal {
    // dma_alloc 在 0.7.x 中是 Safe 的
    fn dma_alloc(pages: usize, _dir: BufferDirection) -> (VirtioPhysAddr, NonNull<u8>) {
        let paddr = virtio_dma_alloc(pages);
        let vaddr = virtio_phys_to_virt(paddr);
        let ptr = NonNull::new(vaddr.0 as *mut u8).unwrap();
        (paddr.0, ptr)
    }

    // 以下方法必须是 unsafe
    unsafe fn dma_dealloc(paddr: VirtioPhysAddr, _vaddr: NonNull<u8>, pages: usize) -> i32 {
        virtio_dma_dealloc(PhysAddr(paddr), pages)
    }

    unsafe fn mmio_phys_to_virt(paddr: VirtioPhysAddr, _size: usize) -> NonNull<u8> {
        let vaddr = virtio_phys_to_virt(PhysAddr(paddr));
        NonNull::new(vaddr.0 as *mut u8).unwrap()
    }

    unsafe fn share(buffer: NonNull<[u8]>, _dir: BufferDirection) -> VirtioPhysAddr {
        let vaddr = VirtAddr(buffer.as_ptr() as *const u8 as usize);
        let paddr = virtio_virt_to_phys(vaddr);
        paddr.0
    }

    unsafe fn unshare(_paddr: VirtioPhysAddr, _buffer: NonNull<[u8]>, _dir: BufferDirection) {
        // 这里的实现依赖于是否使用 IOMMU，如果直接映射则留空
    }
}

// 辅助函数
fn virtio_dma_alloc(pages: usize) -> PhysAddr {
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
    // 必须引入 PageTable trait 才能调用 from_token
    PageTableImpl::from_token(*KERNEL_TOKEN)
        .translate_va(vaddr)
        .unwrap()
}