use core::ptr::NonNull;

use alloc::boxed::Box;

use crate::{
    IOError,
    file::File,
    inode::{FsINodeRef, INode, INodeRef},
    path::ffi::PathBufOpaqueRef,
};

use api_utils::cglue;

#[cglue::cglue_trait]
pub trait Superblock {
    fn get_root_inode_ref(&self) -> FsINodeRef;
    fn get_inode(&self, inode: FsINodeRef) -> Option<INodeRef<'_>>;
    // fn open(&mut self, inode: FsINodeRef) -> Result<Box<dyn File>, IOError>;
    fn unmount(self);
}

#[cglue::cglue_trait]
pub trait Filesystem {
    fn name(&self) -> &str;
    fn mount(&self, device: Option<NonNull<PathBufOpaqueRef>>) -> Option<SuperblockBox<'static>>;
}
