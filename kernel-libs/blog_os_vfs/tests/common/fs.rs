use std::{borrow::Cow, collections::HashMap};

use blog_os_vfs::{
    api::fs::Superblock,
    api::inode::{FsINodeRef, cglue_inode::*},
};
use blog_os_vfs_api::{
    IOError, cglue,
    file::{File, cglue_file::FileBox},
    inode::{INode, cglue_inode::INodeRef},
    stat::Stat,
};

enum CustomINode {
    Regular {
        data: Vec<usize>,
    },
    Directory {
        inodes: HashMap<Cow<'static, str>, FsINodeRef>,
    },
}

impl INode for CustomINode {
    fn lookup(&self, component: &str) -> Option<FsINodeRef> {
        match self {
            Self::Directory { inodes } => inodes.get(component).copied(),
            _ => None,
        }
    }

    fn stat(&self) -> Result<Stat, IOError> {
        todo!()
    }

    fn open(&self) -> Option<FileBox<'_>> {
        todo!()
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

    fn get_inode(&self, inode: FsINodeRef) -> Option<INodeRef> {
        self.inodes
            .get(inode.0 as usize)
            .map(|x| cglue::trait_obj!(x as INode))
    }

    fn unmount(self) {
        todo!()
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
