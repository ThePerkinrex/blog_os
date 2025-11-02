use alloc::boxed::Box;

use crate::{
    IOError,
    file::File,
    inode::{FsINodeRef, INode},
};

pub trait Superblock {
    fn get_root_inode_ref(&self) -> FsINodeRef;
    fn get_inode(&self, inode: FsINodeRef) -> Option<&dyn INode>;
    fn open(&mut self, inode: FsINodeRef) -> Result<Box<dyn File>, IOError>;
}
