use crate::inode::{FsINodeRef, INode};

pub trait Superblock {
    fn get_root_inode_ref(&self) -> FsINodeRef;
    fn get_inode(&self, inode: FsINodeRef) -> &dyn INode;
}
