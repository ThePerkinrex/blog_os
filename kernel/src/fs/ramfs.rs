use alloc::{borrow::Cow, collections::btree_map::BTreeMap, vec::Vec};
use blog_os_vfs::api::{IOError, fs::Superblock, inode::{FsINodeRef, INode, INodeType}, stat::Stat};

enum RamFSINode {
    Regular {
        data: Vec<usize>,
    },
    Directory {
        inodes: BTreeMap<Cow<'static, str>, FsINodeRef>,
    },
}

impl INode for RamFSINode {
    fn get_type(&self) -> INodeType {
        match self {
            Self::Regular { data: _ } => INodeType::RegularFile,
            Self::Directory { inodes: _ } => INodeType::Directory,
        }
    }

    fn lookup(&self, component: &str) -> Option<FsINodeRef> {
        match self {
            Self::Directory { inodes } => inodes.get(component).copied(),
            _ => None,
        }
    }

    fn stat(&self) -> Result<Stat, IOError> {
        todo!()
    }
}

pub struct RamFS {

}

impl Superblock for RamFS {
	fn get_root_inode_ref(&self) -> blog_os_vfs::api::inode::FsINodeRef {
		FsINodeRef(0)
	}

	fn get_inode(&self, inode: blog_os_vfs::api::inode::FsINodeRef) -> Option<&dyn blog_os_vfs::api::inode::INode> {
		todo!()
	}

	fn open(&mut self, inode: blog_os_vfs::api::inode::FsINodeRef) -> Result<alloc::boxed::Box<dyn blog_os_vfs::api::file::File>, blog_os_vfs::api::IOError> {
		todo!()
	}
}