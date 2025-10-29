use crate::block::FsBlockRef;

#[derive(Debug, Clone, Copy)]
pub struct FsINodeRef(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum INodeType {
    RegularFile,
    Directory,
    BlockDevice,
    CharDevice,
    SymbolicLink,
    // Socket
    // FIFO
}

pub trait INode {
    fn get_type(&self) -> INodeType;
    fn get_data_blocks(&self) -> &[FsBlockRef];
    fn lookup(&self, component: &str) -> Option<FsINodeRef>;
}
