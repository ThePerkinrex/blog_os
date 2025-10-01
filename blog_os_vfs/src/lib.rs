#![no_std]

#[derive(Debug, Clone, Copy)]
pub struct FsINodeRef(pub usize);

#[derive(Debug, Clone, Copy)]
pub struct FsIdx(usize);

pub trait INode {
	
}


pub trait Superblock {
	fn get_root_inode_ref(&self) -> FsINodeRef;
	fn get_inode(&self, inode: FsINodeRef) -> &dyn INode;
}


#[derive(Debug, Clone, Copy)]
pub struct INodeRef(FsIdx, FsINodeRef);



pub struct VFS {
	// TODO alloc needed
	root_fs: FsIdx,
	
}