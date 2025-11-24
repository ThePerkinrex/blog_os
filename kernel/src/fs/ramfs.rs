use core::ptr::NonNull;

use alloc::{boxed::Box, collections::btree_map::BTreeMap, string::String, sync::Arc, vec::Vec};
use api_utils::cglue;
use blog_os_device::api::DeviceId;
use blog_os_vfs::api::{
    IOError,
    file::{File, cglue_file::*},
    fs::{Filesystem, Superblock, cglue_superblock::*},
    inode::{FsINodeRef, INode, cglue_inode::*},
    path::ffi::PathBufOpaqueRef,
    stat::Stat,
};
use slotmap::KeyData;
use spin::RwLock;

slotmap::new_key_type! {struct INodeKey;}

struct RamFSSuperblock {
    root_inode: INodeKey,
    inodes: slotmap::SlotMap<INodeKey, INodeBox<'static>>,
}

impl RamFSSuperblock {
    fn new() -> Self {
        let mut inodes = slotmap::SlotMap::with_key();
        let root_inode = inodes.insert(cglue::trait_obj!(DirectoryINode::default() as INode));
        Self { root_inode, inodes }
    }
}

impl Default for RamFSSuperblock {
    fn default() -> Self {
        Self::new()
    }
}

impl Superblock for RamFSSuperblock {
    fn get_root_inode_ref(&self) -> FsINodeRef {
        FsINodeRef(self.root_inode.0.as_ffi())
    }

    fn get_inode(&self, inode: FsINodeRef) -> Option<INodeRef<'_>> {
        let key = INodeKey::from(KeyData::from_ffi(inode.0));

        self.inodes.get(key).map(|x| cglue::trait_obj!(x as INode))
    }

    fn unmount(self) {
        drop(self);
    }
}

pub struct RamFS;

impl Filesystem for RamFS {
    fn name(&self) -> &str {
        "ramfs"
    }

    fn mount(&self, device: Option<NonNull<PathBufOpaqueRef>>) -> Option<SuperblockBox<'static>> {
        if device.is_some() {
            None
        } else {
            Some(cglue::trait_obj!(RamFSSuperblock::default() as Superblock))
        }
    }
}

#[derive(Debug, Default)]
struct DirectoryINode {
    entries: Arc<RwLock<BTreeMap<String, INodeKey>>>,
}

impl INode for DirectoryINode {
    fn lookup(&self, component: &str) -> Option<FsINodeRef> {
        self.entries
            .read()
            .get(component)
            .map(|x| FsINodeRef(x.0.as_ffi()))
    }

    fn stat(&self) -> Result<Stat, IOError> {
        Ok(Stat {
            device: None,
            size: self.entries.read().len() as u64,
            file_type: blog_os_vfs::api::stat::FileType::Directory,
        })
    }

    fn open(&self) -> Option<FileBox<'_>> {
        Some(cglue::trait_obj!(DirectoryFile { inode: self } as File))
    }
}

struct DirectoryFile<'a> {
    inode: &'a DirectoryINode,
}

impl<'a> File for DirectoryFile<'a> {
    fn close(self) -> Result<(), IOError> {
        Ok(())
    }

    fn read(&mut self, _: &mut [u8]) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn write(&mut self, _: &[u8]) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn readdir(&self) -> Result<Vec<Box<str>>, IOError> {
        Ok(self
            .inode
            .entries
            .read()
            .keys()
            .map(|x| Box::from(x.as_str()))
            .collect())
    }

    fn mkdir(&mut self, name: &str) -> Result<FsINodeRef, IOError> {
        todo!()
    }

    fn mknod(&mut self, name: &str, device: DeviceId) -> Result<FsINodeRef, IOError> {
        todo!()
    }

    fn creat(&mut self, name: &str) -> Result<FsINodeRef, IOError> {
        todo!()
    }
}
