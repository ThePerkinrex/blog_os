use crate::{IOError, file::FileBox, stat::Stat};

use api_utils::cglue;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct FsINodeRef(pub u64);

#[cglue::cglue_trait]
pub trait INode {
    // fn get_type(&self) -> INodeType;
    fn lookup(&self, component: &str) -> Option<FsINodeRef>;
    fn stat(&self) -> Result<Stat, IOError>;
    fn open(&self) -> Result<FileBox<'_>, IOError>;
}
