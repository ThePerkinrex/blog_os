use api_utils::cglue;
use blog_os_vfs::{
    VFS,
    api::{fs::cglue_filesystem::*, path::PathBuf},
};
use ramfs::fs::{RAMFS_TYPE, RamFS};
use spin::{Lazy, lock_api::RwLock};

pub static VFS: Lazy<RwLock<VFS>> = Lazy::new(|| RwLock::new(VFS::new()));

pub fn init_ramfs() {
    let ramfs = RamFS::<spin::RwLock<()>>::default();

    let mut lock = VFS.write();

    lock.register_fs(cglue::trait_obj!(ramfs as Filesystem))
        .unwrap();
    lock.mount_type(PathBuf::root(), None, RAMFS_TYPE);
}
