use api_utils::cglue;
use blog_os_vfs::{
    VFS,
    api::{fs::cglue_filesystem::*, path::PathBuf},
};
use log::debug;
use ramfs::fs::{RAMFS_TYPE, RamFS};
use spin::{Lazy, lock_api::RwLock};
use x86_64::{
    VirtAddr,
    structures::paging::{FrameDeallocator, Mapper, Page, Size4KiB},
};

use crate::setup::KERNEL_INFO;

pub static VFS: Lazy<RwLock<VFS>> = Lazy::new(|| RwLock::new(VFS::new()));

pub fn init_ramfs() {
    let ramfs = RamFS::<spin::RwLock<()>>::default();

    let mut lock = VFS.write();

    lock.register_fs(cglue::trait_obj!(ramfs as Filesystem))
        .unwrap();
    lock.mount_type(PathBuf::root(), None, RAMFS_TYPE);

    let kinf = KERNEL_INFO.get().unwrap();

    if let Some(ramdisk_addr) = {
        let mut lock = kinf.ramdisk_addr.lock();
        let taken = lock.take();
        drop(lock);
        taken
    } {
        let ramdisk_addr = VirtAddr::new_truncate(ramdisk_addr);
        let ramdisk_len = kinf.ramdisk_len as usize;

        let data = unsafe { core::slice::from_raw_parts(ramdisk_addr.as_ptr::<u8>(), ramdisk_len) };

        initcpio::load_initcpio(&mut lock, data);

        // Clear cpio pages
        let range = Page::<Size4KiB>::range_inclusive(
            Page::containing_address(ramdisk_addr),
            Page::containing_address(ramdisk_addr + (kinf.ramdisk_len - 1)),
        );
        let mut alloc = kinf.alloc_kinf.lock();
        for p in range {
            let (frame, flush) = alloc.page_table.unmap(p).unwrap();
            flush.flush();
            unsafe { alloc.frame_allocator.deallocate_frame(frame) };
        }
        drop(alloc);

        debug!("Freed {} pages/frames", range.count())
    }

    drop(lock);
}
