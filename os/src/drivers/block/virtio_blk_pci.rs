use super::BlockDevice;
use crate::mm::{
    frame_alloc, frame_dealloc, frames_alloc, kernel_token, FrameTracker, PageTable, PageTableImpl, PhysAddr,
    PhysPageNum, StepByOne, VirtAddr,
};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ptr::NonNull;
use lazy_static::*;
use spin::Mutex;
use virtio_drivers::{BufferDirection, Hal, PhysAddr as VirtioPhysAddr};
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::pci::bus::{BarInfo, Cam, Command, MemoryBarType, PciRoot};
use virtio_drivers::transport::pci::{PciTransport, virtio_device_type};
use virtio_drivers::transport::DeviceType;

use crate::hal::config::{BLOCK_SZ, PAGE_SIZE, PAGE_SIZE_BITS};

// VirtIO 块大小通常固定为 512
const VIRT_IO_BLOCK_SZ: usize = 512;
const BLOCK_RATIO: usize = BLOCK_SZ / VIRT_IO_BLOCK_SZ;

// LoongArch QEMU 平台的 PCI ECAM 基地址
const PCI_ECAM_BASE: usize = 0x2000_0000;
const VIRT_PCI_BASE: usize = 0x4000_0000;
const VIRT_PCI_SIZE: usize = 0x0002_0000;

pub struct VirtIOBlock(Mutex<VirtIOBlk<VirtioHal, PciTransport>>);

lazy_static! {
    static ref QUEUE_FRAMES: Mutex<Vec<Arc<FrameTracker>>> = Mutex::new(Vec::new());
}

impl BlockDevice for VirtIOBlock {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        assert!(buf.len() % BLOCK_SZ == 0);
        for (i, chunk) in buf.chunks_mut(VIRT_IO_BLOCK_SZ).enumerate() {
            let virtio_block_id = block_id * BLOCK_RATIO + i;
            self.0
                .lock()
                .read_blocks(virtio_block_id, chunk)
                .expect("read error");
        }
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        assert!(buf.len() % BLOCK_SZ == 0);
        for (i, chunk) in buf.chunks(VIRT_IO_BLOCK_SZ).enumerate() {
            let virtio_block_id = block_id * BLOCK_RATIO + i;
            self.0
                .lock()
                .write_blocks(virtio_block_id, chunk)
                .expect("write error");
        }
    }
}

pub struct PciRangeAllocator {
    end: usize,
    current: usize,
}

impl PciRangeAllocator {
    pub const fn new(pci_base: usize, pci_size: usize) -> Self {
        Self {
            current: pci_base,
            end: pci_base + pci_size,
        }
    }

    pub fn alloc_pci_mem(&mut self, size: usize) -> Option<usize> {
        if !size.is_power_of_two() {
            return None;
        }
        let ret = align_up(self.current, size);
        if ret + size > self.end {
            return None;
        }
        self.current = ret + size;
        Some(ret & !0xf)
    }
}

const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

fn enumerate_pci() -> Option<PciTransport> {
    let mmconfig_base = PCI_ECAM_BASE as *mut u8;
    println!("[PCI] ECAM base: {:#x}", mmconfig_base as usize);

    // unsafe 创建 PciRoot
    let mut pci_root = unsafe { PciRoot::new(mmconfig_base, Cam::Ecam) };
    let mut transport = None;

    for (device_function, info) in pci_root.enumerate_bus(0) {
        let (vendor, device) = (info.vendor_id, info.device_id);
        if vendor == 0xffff { continue; } 

        if let Some(virtio_type) = virtio_device_type(&info) {
            if virtio_type != DeviceType::Block {
                continue;
            }
            println!("[PCI] Found VirtIO Block Device: {:?} {:?} ", device_function, info);

            let mut allocator = PciRangeAllocator::new(VIRT_PCI_BASE, VIRT_PCI_SIZE);
            for i in 0..6 {
                // 修复: 使用 if let Ok(bar) 匹配 Result
                if let Ok(bar) = pci_root.bar_info(device_function, i) {
                    if let BarInfo::Memory { size, address_type, .. } = bar {
                        if size > 0 {
                            if let Some(addr) = allocator.alloc_pci_mem(size as usize) {
                                match address_type {
                                    MemoryBarType::Width64 => pci_root.set_bar_64(device_function, i, addr as u64),
                                    MemoryBarType::Width32 => pci_root.set_bar_32(device_function, i, addr as u32),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            pci_root.set_command(
                device_function,
                Command::IO_SPACE | Command::MEMORY_SPACE | Command::BUS_MASTER,
            );
            
            if let Ok(t) = PciTransport::new::<VirtioHal>(&mut pci_root, device_function) {
                println!("[PCI] Device initialized successfully.");
                transport = Some(t);
                break;
            }
        }
    }
    transport
}

impl VirtIOBlock {
    pub fn new() -> Self {
        let transport = enumerate_pci().expect("No VirtIO block device found on PCI bus");
        let virtio_blk = VirtIOBlk::<VirtioHal, PciTransport>::new(transport).expect("Failed to create VirtIOBlk");
        Self(Mutex::new(virtio_blk))
    }
}

pub struct VirtioHal;

unsafe impl Hal for VirtioHal {
    fn dma_alloc(pages: usize, _dir: BufferDirection) -> (VirtioPhysAddr, NonNull<u8>) {
        let paddr = virtio_dma_alloc(pages);
        let vaddr = virtio_phys_to_virt(paddr);
        let ptr = NonNull::new(vaddr.0 as *mut u8).unwrap();
        (paddr.0, ptr)
    }

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
    }
}

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
    let mut ppn = pa.into();
    for _ in 0..pages {
        frame_dealloc(ppn);
        ppn.step();
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
    // 引入 PageTable trait 后，可以调用 from_token
    PageTableImpl::from_token(*KERNEL_TOKEN)
        .translate_va(vaddr)
        .unwrap()
}