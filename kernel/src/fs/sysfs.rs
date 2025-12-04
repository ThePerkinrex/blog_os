use core::ptr::NonNull;

use api_utils::cglue::{self, arc::CArc};
use blog_os_vfs::api::{
    fs::{Filesystem, Superblock, cglue_superblock::*},
    inode::{FsINodeRef, cglue_inode::INodeBox},
    path::ffi::PathBufOpaqueRef,
};

pub struct SysFs;

impl Filesystem for SysFs {
    fn name(&self) -> &str {
        "sysfs"
    }

    fn mount(&self, device: Option<NonNull<PathBufOpaqueRef>>) -> Option<SuperblockBox<'static>> {
        if device.is_some() {
            None
        } else {
            Some(cglue::trait_obj!(SysFsSuperblock as Superblock))
        }
    }
}

pub struct SysFsSuperblock;

impl Superblock for SysFsSuperblock {
    fn get_root_inode_ref(&self) -> FsINodeRef {
        todo!()
    }

    fn get_inode(&self, inode: FsINodeRef) -> CArc<INodeBox<'static>> {
        todo!()
    }

    fn unmount(self) {
        todo!()
    }
}
