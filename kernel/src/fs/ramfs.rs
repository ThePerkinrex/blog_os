use alloc::{borrow::Cow, boxed::Box, collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
use blog_os_vfs::api::{
    IOError,
    fs::Superblock,
    inode::{FsINodeRef, INode, INodeType},
    stat::Stat,
};
use slotmap::{Key, KeyData, SlotMap};
use spin::lock_api::RwLock;

struct RegularRamFSINode {
    data: Vec<u8>,
}

struct DirectoryRamFSINode {
    inodes: BTreeMap<Cow<'static, str>, FsINodeRef>,
}

enum RamFSINode {
    Regular(RwLock<RegularRamFSINode>),
    Directory(RwLock<DirectoryRamFSINode>),
}

impl INode for RamFSINode {
    fn get_type(&self) -> INodeType {
        match self {
            Self::Regular(_) => INodeType::RegularFile,
            Self::Directory(_) => INodeType::Directory,
        }
    }

    fn lookup(&self, component: &str) -> Option<FsINodeRef> {
        match self {
            Self::Directory(inodes) => inodes.read().inodes.get(component).copied(),
            _ => None,
        }
    }

    fn stat(&self) -> Result<Stat, IOError> {
        match self {
            Self::Regular(regular) => Ok(Stat {
                device: None,
                size: regular.read().data.len() as u64,
            }),
            Self::Directory(directory) => Ok(Stat {
                device: None,
                size: directory.read().inodes.len() as u64,
            }),
        }
    }
}

slotmap::new_key_type! {
    pub struct RamFSINodeIdx;
}

impl From<FsINodeRef> for RamFSINodeIdx {
    fn from(value: FsINodeRef) -> Self {
        KeyData::from_ffi(value.0).into()
    }
}

impl From<RamFSINodeIdx> for FsINodeRef {
    fn from(value: RamFSINodeIdx) -> Self {
        Self(value.data().as_ffi())
    }
}

struct RamFSFile {
    cursor: usize,
    inode: Arc<RamFSINode>,
}

impl RamFSFile {
    pub const fn new(inode: Arc<RamFSINode>) -> Self {
        Self { cursor: 0, inode }
    }
}

impl blog_os_vfs::api::file::File for RamFSFile {
    fn close(self) -> Result<(), IOError> {
        drop(self);
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IOError> {
        match self.inode.as_ref() {
            RamFSINode::Regular(rw_lock) => {
                let data = &rw_lock.read().data[self.cursor..];
                let m = data.len().min(buf.len());

                buf[..m].copy_from_slice(&data[..m]);

                self.cursor += m;
                Ok(m)
            }
            _ => Err(IOError::OperationNotPermitted),
        }
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, IOError> {
        match self.inode.as_ref() {
            RamFSINode::Regular(rw_lock) => {
                let data = &mut rw_lock.write().data[self.cursor..];
                let m = data.len().min(buf.len());

                data[..m].copy_from_slice(&buf[..m]);

                self.cursor += m;
                Ok(m)
            }
            _ => Err(IOError::OperationNotPermitted),
        }
    }

    fn readdir(&self) -> Result<Vec<alloc::boxed::Box<str>>, IOError> {
        match self.inode.as_ref() {
            RamFSINode::Directory(rw_lock) => Ok(rw_lock
                .read()
                .inodes
                .keys()
                .map(|x| Box::from(x.as_ref()))
                .collect()),
            _ => Err(IOError::OperationNotPermitted),
        }
    }

    fn mkdir(&mut self, name: &str) -> Result<FsINodeRef, IOError> {
        match self.inode.as_ref() {
            RamFSINode::Directory(rw_lock) => {
                todo!()
            }
            _ => Err(IOError::OperationNotPermitted),
        }
    }

    fn mknod(
        &mut self,
        name: &str,
        device: blog_os_device::api::DeviceId,
    ) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn creat(&mut self, name: &str) -> Result<FsINodeRef, IOError> {
        todo!()
    }
}

pub struct RamFS {
    inodes: SlotMap<RamFSINodeIdx, Arc<RamFSINode>>,
    root: RamFSINodeIdx,
}

impl Superblock for RamFS {
    fn get_root_inode_ref(&self) -> blog_os_vfs::api::inode::FsINodeRef {
        self.root.into()
    }

    fn get_inode(
        &self,
        inode: blog_os_vfs::api::inode::FsINodeRef,
    ) -> Option<&dyn blog_os_vfs::api::inode::INode> {
        let idx = RamFSINodeIdx::from(inode);
        self.inodes.get(idx).map(|x| x.as_ref() as &dyn INode)
    }

    fn open(
        &mut self,
        inode: blog_os_vfs::api::inode::FsINodeRef,
    ) -> Result<alloc::boxed::Box<dyn blog_os_vfs::api::file::File>, blog_os_vfs::api::IOError>
    {
        let idx = RamFSINodeIdx::from(inode);
        self.inodes
            .get(idx)
            .map(|x| Box::new(RamFSFile::new(x.clone())) as Box<dyn blog_os_vfs::api::file::File>)
            .ok_or(IOError::NotFound)
    }
}
