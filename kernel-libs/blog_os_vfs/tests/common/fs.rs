use std::{borrow::Cow, collections::HashMap};

use blog_os_vfs::{
    block::{Block, FsBlockRef},
    fs::Superblock,
    inode::{FsINodeRef, INode, INodeType},
};

enum CustomINode {
    Regular {
        data: Vec<FsBlockRef>,
    },
    Directory {
        inodes: HashMap<Cow<'static, str>, FsINodeRef>,
    },
}

impl INode for CustomINode {
    fn get_type(&self) -> INodeType {
        match self {
            Self::Regular { data: _ } => INodeType::RegularFile,
            Self::Directory { inodes: _ } => INodeType::Directory,
        }
    }

    fn get_data_blocks(&self) -> &[FsBlockRef] {
        match self {
            Self::Regular { data } => data.as_slice(),
            _ => &[],
        }
    }

    fn lookup(&self, component: &str) -> Option<FsINodeRef> {
        match self {
            Self::Directory { inodes } => inodes.get(component).copied(),
            _ => None,
        }
    }
}

pub struct CustomFs {
    data_blocks: Vec<[u8; 4096]>,
    inodes: Vec<CustomINode>,
}

impl Superblock for CustomFs {
    fn get_root_inode_ref(&self) -> FsINodeRef {
        FsINodeRef(0)
    }

    fn get_inode(&self, inode: FsINodeRef) -> Option<&dyn INode> {
        self.inodes.get(inode.0).map(|x| x as &dyn INode)
    }

    fn get_block(&self, block: FsBlockRef) -> Option<&Block> {
        self.data_blocks.get(block.0).map(|x| x.as_slice())
    }

    fn get_mut_block(&mut self, block: FsBlockRef) -> Option<&mut Block> {
        self.data_blocks.get_mut(block.0).map(|x| x.as_mut_slice())
    }
}

pub fn example_fs_empty_files() -> CustomFs {
    CustomFs {
        data_blocks: vec![],
        inodes: vec![
            CustomINode::Directory {
                inodes: {
                    let mut hm = HashMap::new();

                    hm.insert("bin".into(), FsINodeRef(1));

                    hm
                },
            },
            CustomINode::Directory {
                inodes: {
                    let mut hm = HashMap::new();

                    hm.insert("sh".into(), FsINodeRef(2));
                    hm.insert("echo".into(), FsINodeRef(3));

                    hm
                },
            },
            CustomINode::Regular { data: vec![] },
            CustomINode::Regular { data: vec![] },
        ],
    }
}
