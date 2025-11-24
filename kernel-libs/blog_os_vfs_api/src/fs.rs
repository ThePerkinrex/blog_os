use alloc::boxed::Box;

use crate::{
    IOError,
    file::File,
    inode::{FsINodeRef, INode},
};

use api_utils::cglue;


#[cglue::cglue_trait]
pub trait Superblock {
    fn get_root_inode_ref(&self) -> FsINodeRef;
    fn get_inode(&self, inode: FsINodeRef) -> Option<&dyn INode>;
    fn open(&mut self, inode: FsINodeRef) -> Result<Box<dyn File>, IOError>;
}

#[cglue::cglue_trait]
pub trait Filesystem {
    fn name(&self) -> &str;
    fn mount(&self) -> SuperblockBox<'static>; // TODO device/data
}