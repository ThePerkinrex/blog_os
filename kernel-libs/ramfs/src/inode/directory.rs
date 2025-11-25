use alloc::{boxed::Box, collections::btree_map::BTreeMap, string::String, sync::Arc, vec::Vec};
use api_utils::cglue;
use blog_os_device::api::DeviceId;
use blog_os_vfs::api::{
    IOError,
    file::{File, cglue_file::*},
    inode::{FsINodeRef, INode, cglue_inode::*},
    stat::Stat,
};
use lock_api::{RawRwLock, RwLock};
use slotmap::Key;

use crate::superblock::{INodeKey, SharedSuperblockData};

pub struct DirectoryINode<R: RawRwLock + Send + Sync + 'static> {
    entries: Arc<RwLock<R, BTreeMap<String, INodeKey>>>,
    superblock: Arc<SharedSuperblockData<R>>,
}

impl<R: RawRwLock + Send + Sync> DirectoryINode<R> {
    pub(crate) fn new(superblock: Arc<SharedSuperblockData<R>>) -> Self {
        Self {
            entries: Default::default(),
            superblock,
        }
    }
}

impl<R: RawRwLock + Send + Sync> INode for DirectoryINode<R> {
    fn lookup(&self, component: &str) -> Option<FsINodeRef> {
        self.entries
            .read()
            .get(component)
            .map(|x| FsINodeRef(x.data().as_ffi()))
    }

    fn stat(&self) -> Result<Stat, IOError> {
        Ok(Stat {
            device: None,
            size: self.entries.read().len() as u64,
            file_type: blog_os_vfs::api::stat::FileType::Directory,
        })
    }

    fn open(&self) -> Option<FileBox<'_>> {
        Some(cglue::trait_obj!(
            DirectoryFile::<'_, R> { inode: self } as File
        ))
    }
}

pub struct DirectoryFile<'a, R: RawRwLock + Send + Sync + 'static> {
    inode: &'a DirectoryINode<R>,
}

impl<'a, R: RawRwLock + Send + Sync> File for DirectoryFile<'a, R> {
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
        if self.inode.entries.read().contains_key(name) {
            return Err(IOError::AlreadyExists);
        }

        let inode = Arc::new(cglue::trait_obj!(
            DirectoryINode::new(self.inode.superblock.clone()) as INode
        ));

        let key = self.inode.superblock.inodes.write().insert(inode);

        self.inode.entries.write().insert(name.into(), key);

        Ok(FsINodeRef(key.data().as_ffi()))
    }

    fn mknod(&mut self, name: &str, _device: DeviceId) -> Result<FsINodeRef, IOError> {
        if self.inode.entries.read().contains_key(name) {
            return Err(IOError::AlreadyExists);
        }

        todo!()
    }

    fn creat(&mut self, name: &str) -> Result<FsINodeRef, IOError> {
        if self.inode.entries.read().contains_key(name) {
            return Err(IOError::AlreadyExists);
        }

        todo!()
    }
}
