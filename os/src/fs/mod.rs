//! Filesystem subsystem
//!
//! This module provides:
//! - Virtual filesystem (VFS) abstraction
//! - FAT32 and EXT4 filesystem support
//! - File descriptor management
//! - Directory tree structure
//! - Device file support (pipe, null, zero, etc.)
//! - Page cache for file I/O
//! - Swap file support (optional)

mod cache;
pub mod dev;
pub mod directory_tree;
mod ext4;
pub mod fat32;
pub mod file_trait;
mod filesystem;
mod layout;
pub mod poll;
#[cfg(feature = "swap")]
pub mod swap;
pub mod dirent;
pub mod file_descriptor;
mod inode;
mod timestamp;
mod vfs;


pub use self::dev::{
    hwclock::*,
    interrupts::*,
    // null::*,
    pipe::*,
    // socket::*, tty::*, zero::*
};

pub use self::layout::*;

pub use self::fat32::DiskInodeType;
pub use crate::drivers::block::BlockDevice;

use self::cache::PageCache;
use alloc::{
    string::String,
    sync::Arc,
};
pub use dirent::Dirent;
pub use file_descriptor::FileDescriptor;
use lazy_static::*;

lazy_static! {
    /// Root directory file descriptor
    ///
    /// Provides access to the filesystem root directory for all file operations
    pub static ref ROOT_FD: Arc<FileDescriptor> = Arc::new(FileDescriptor::new(
        false,
        false,
        self::directory_tree::ROOT
            .open(".", OpenFlags::O_RDONLY | OpenFlags::O_DIRECTORY, true)
            .unwrap()
    ));
}

/// Flush preloaded binaries to filesystem
///
/// Writes initproc and bash binaries from memory to the filesystem
/// and deallocates the memory used for preloading
#[allow(unused)]
pub fn flush_preload() {
    extern "C" {
        fn sinitproc();
        fn einitproc();
        fn sbash();
        fn ebash();
    }
    println!(
        "sinitproc: {:X}, einitproc: {:X}, sbash: {:X}, ebash: {:X}",
        sinitproc as usize, einitproc as usize, sbash as usize, ebash as usize,
    );
    let initproc = ROOT_FD.open("initproc", OpenFlags::O_CREAT, false).unwrap();
    initproc.write(None, unsafe {
        core::slice::from_raw_parts(
            sinitproc as *const u8,
            einitproc as usize - sinitproc as usize,
        )
    });
    for ppn in crate::mm::PPNRange::new(
        crate::mm::PhysAddr::from(sinitproc as usize).floor(),
        crate::mm::PhysAddr::from(einitproc as usize).floor(),
    ) {
        crate::mm::frame_dealloc(ppn);
    }
    let bash = ROOT_FD.open("bash", OpenFlags::O_CREAT, false).unwrap();
    bash.write(None, unsafe {
        core::slice::from_raw_parts(sbash as *const u8, ebash as usize - sbash as usize)
    });
    for ppn in crate::mm::PPNRange::new(
        crate::mm::PhysAddr::from(sbash as usize).floor(),
        crate::mm::PhysAddr::from(ebash as usize).floor(),
    ) {
        crate::mm::frame_dealloc(ppn);
    }
}
