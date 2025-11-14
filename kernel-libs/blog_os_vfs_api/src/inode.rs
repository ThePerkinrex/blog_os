use crate::{IOError, stat::Stat};

#[derive(Debug, Clone, Copy)]
pub struct FsINodeRef(pub u64);

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
    fn lookup(&self, component: &str) -> Option<FsINodeRef>;
    fn stat(&self) -> Result<Stat, IOError>;
}
