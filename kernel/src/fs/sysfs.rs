use core::ptr::NonNull;

use alloc::sync::Arc;
use api_utils::cglue::{self, arc::CArc};
use blog_os_vfs::api::{
    fs::{Filesystem, Superblock, cglue_superblock::*},
    inode::{FsINodeRef, cglue_inode::*},
    path::ffi::PathBufOpaqueRef,
};
use slotmap::{Key, KeyData};
use spin::lock_api::RwLock;

use crate::fs::sysfs::root::RootINode;

mod device;
mod driver;
mod proc;
mod root;

pub struct SysFs;

impl Filesystem for SysFs {
    fn name(&self) -> &str {
        "sysfs"
    }

    fn mount(&self, device: Option<NonNull<PathBufOpaqueRef>>) -> Option<SuperblockBox<'static>> {
        if device.is_some() {
            None
        } else {
            Some(cglue::trait_obj!(SysFsSuperblock::default() as Superblock))
        }
    }
}

slotmap::new_key_type! {struct SysFsINode;}

type INodes = Arc<RwLock<slotmap::SlotMap<SysFsINode, Arc<INodeBox<'static>>>>>;

pub struct SysFsSuperblock {
    root_inode: SysFsINode,
    inodes: INodes,
}

impl SysFsSuperblock {
    pub fn new() -> Self {
        let inodes: INodes = Default::default();
        let root_inode =
            inodes.write().insert(Arc::new(cglue::trait_obj!(
                RootINode::new(inodes.clone()) as INode
            )));

        Self { root_inode, inodes }
    }
}

impl Default for SysFsSuperblock {
    fn default() -> Self {
        Self::new()
    }
}

impl Superblock for SysFsSuperblock {
    fn get_root_inode_ref(&self) -> FsINodeRef {
        FsINodeRef(self.root_inode.data().as_ffi())
    }

    fn get_inode(&self, inode: FsINodeRef) -> CArc<INodeBox<'static>> {
        let inode = SysFsINode::from(KeyData::from_ffi(inode.0));

        self.inodes.read().get(inode).cloned().into()
    }

    fn unmount(self) {}
}
