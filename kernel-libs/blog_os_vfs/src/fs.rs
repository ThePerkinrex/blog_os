use crate::{
    block::{Block, FsBlockRef},
    inode::{FsINodeRef, INode},
};

pub trait Superblock {
    fn get_root_inode_ref(&self) -> FsINodeRef;
    fn get_inode(&self, inode: FsINodeRef) -> Option<&dyn INode>;
    fn get_block(&self, block: FsBlockRef) -> Option<&Block>;
    fn get_mut_block(&mut self, block: FsBlockRef) -> Option<&mut Block>;
}
