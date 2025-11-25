use api_utils::cglue;
use blog_os_vfs::{
    VFS,
    api::{fs::cglue_filesystem::*, path::PathBuf},
};
use ramfs::fs::{RAMFS_TYPE, RamFS};
use spin::{Lazy, lock_api::RwLock};
use x86_64::VirtAddr;

use crate::setup::KERNEL_INFO;

pub static VFS: Lazy<RwLock<VFS>> = Lazy::new(|| RwLock::new(VFS::new()));

pub fn init_ramfs() {
    let ramfs = RamFS::<spin::RwLock<()>>::default();

    let mut lock = VFS.write();

    lock.register_fs(cglue::trait_obj!(ramfs as Filesystem))
        .unwrap();
    lock.mount_type(PathBuf::root(), None, RAMFS_TYPE);

    if let Some(ramdisk_addr) = KERNEL_INFO.get().unwrap().ramdisk_addr {
        let ramdisk_addr = VirtAddr::new_truncate(ramdisk_addr);
        let ramdisk_len = KERNEL_INFO.get().unwrap().ramdisk_len as usize;

        let data = unsafe { core::slice::from_raw_parts(ramdisk_addr.as_ptr::<u8>(), ramdisk_len) };

        initcpio::load_initcpio(&mut lock, data);
    }

    drop(lock);
}
