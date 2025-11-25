use core::ptr::NonNull;

use crate::{
    inode::{FsINodeRef, INodeBox},
    path::ffi::PathBufOpaqueRef,
};

use api_utils::cglue::{self, arc::CArc};

#[cglue::cglue_trait]
pub trait Superblock {
    fn get_root_inode_ref(&self) -> FsINodeRef;
    fn get_inode(&self, inode: FsINodeRef) -> CArc<INodeBox<'static>>;
    // fn open(&mut self, inode: FsINodeRef) -> Result<Box<dyn File>, IOError>;
    fn unmount(self);
}

#[cglue::cglue_trait]
pub trait Filesystem {
    fn name(&self) -> &str;
    fn mount(&self, device: Option<NonNull<PathBufOpaqueRef>>) -> Option<SuperblockBox<'static>>;
}
