use std::{borrow::Cow, collections::HashMap, ptr::NonNull, sync::Arc};

use blog_os_vfs::{
    api::fs::Superblock,
    api::inode::{FsINodeRef, cglue_inode::*},
};
use blog_os_vfs_api::{
    IOError,
    cglue::{self, arc::CArc},
    file::cglue_file::FileBox,
    fs::{Filesystem, cglue_superblock::*},
    inode::INode,
    path::ffi::PathBufOpaqueRef,
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

    fn open(&self) -> Result<FileBox<'static>, IOError> {
        todo!()
    }
}

pub struct CustomFsSuperblock {
    data_blocks: Vec<[u8; 4096]>,
    inodes: Vec<Arc<INodeBox<'static>>>,
}

impl Superblock for CustomFsSuperblock {
    fn get_root_inode_ref(&self) -> FsINodeRef {
        FsINodeRef(0)
    }

    fn get_inode(&self, inode: FsINodeRef) -> CArc<INodeBox<'static>> {
        self.inodes.get(inode.0 as usize).cloned().into()
    }

    fn unmount(self) {
        todo!()
    }
}

fn example_fs_empty_files() -> CustomFsSuperblock {
    CustomFsSuperblock {
        data_blocks: vec![],
        inodes: vec![
            Arc::new(cglue::trait_obj!(CustomINode::Directory {
                inodes: {
                    let mut hm = HashMap::new();

                    hm.insert("bin".into(), FsINodeRef(1));

                    hm
                },
            } as INode)),
            Arc::new(cglue::trait_obj!(CustomINode::Directory {
                inodes: {
                    let mut hm = HashMap::new();

                    hm.insert("sh".into(), FsINodeRef(2));
                    hm.insert("echo".into(), FsINodeRef(3));

                    hm
                },
            } as INode)),
            Arc::new(cglue::trait_obj!(
                CustomINode::Regular { data: vec![] } as INode
            )),
            Arc::new(cglue::trait_obj!(
                CustomINode::Regular { data: vec![] } as INode
            )),
        ],
    }
}

pub struct CustomFs;

impl Filesystem for CustomFs {
    fn name(&self) -> &str {
        "customfs"
    }

    fn mount(&self, device: Option<NonNull<PathBufOpaqueRef>>) -> Option<SuperblockBox<'static>> {
        if device.is_some() {
            None
        } else {
            Some(cglue::trait_obj!(example_fs_empty_files() as Superblock))
        }
    }
}
