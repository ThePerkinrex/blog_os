use core::ops::Bound;

use alloc::{boxed::Box, collections::btree_map::BTreeMap, string::String, sync::Arc, vec::Vec};
use api_utils::cglue;
use blog_os_device_api::DeviceId;
use blog_os_vfs_api::{
    IOError,
    file::{File, SeekMode, cglue_file::*},
    inode::{FsINodeRef, INode, cglue_inode::*},
};
use lock_api::{RawRwLock, RwLock};
use shared_fs::{FileType, Stat};
use slotmap::Key;

use crate::{
    inode::regular::RegularINode,
    superblock::{INodeKey, SharedSuperblockData},
};

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
            file_type: FileType::Directory,
        })
    }

    fn open(&self) -> Result<FileBox<'static>, IOError> {
        Ok(cglue::trait_obj!(DirectoryFile::<R> {
            entries: self.entries.clone(),
            superblock: self.superblock.clone(),
            current: None
        } as File))
    }
}

pub struct DirectoryFile<R: RawRwLock + Send + Sync + 'static> {
    entries: Arc<RwLock<R, BTreeMap<String, INodeKey>>>,
    superblock: Arc<SharedSuperblockData<R>>,
    current: Option<String>,
}

impl<R: RawRwLock + Send + Sync + 'static> DirectoryFile<R> {
    fn get_range(&self) -> (Bound<&str>, Bound<&str>) {
        self.current
            .as_deref()
            .map_or((Bound::Unbounded, Bound::Unbounded), |s| {
                (Bound::Excluded(s), Bound::Unbounded)
            })
    }
}

impl<R: RawRwLock + Send + Sync> File for DirectoryFile<R> {
    fn close(&mut self) -> Result<(), IOError> {
        Ok(())
    }

    fn read(&mut self, _: &mut [u8]) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn write(&mut self, _: &[u8]) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    // fn readdir(&self) -> Result<Vec<Box<str>>, IOError> {
    //     Ok(self
    //         .entries
    //         .read()
    //         .keys()
    //         .map(|x| Box::from(x.as_str()))
    //         .collect())
    // }

    fn mkdir(&mut self, name: &str) -> Result<FsINodeRef, IOError> {
        if self.entries.read().contains_key(name) {
            return Err(IOError::AlreadyExists);
        }

        let inode = Arc::new(cglue::trait_obj!(
            DirectoryINode::new(self.superblock.clone()) as INode
        ));

        let key = self.superblock.inodes.write().insert(inode);

        self.entries.write().insert(name.into(), key);

        Ok(FsINodeRef(key.data().as_ffi()))
    }

    fn mknod(&mut self, name: &str, _device: DeviceId) -> Result<FsINodeRef, IOError> {
        if self.entries.read().contains_key(name) {
            return Err(IOError::AlreadyExists);
        }

        todo!()
    }

    fn creat(&mut self, name: &str) -> Result<FsINodeRef, IOError> {
        if self.entries.read().contains_key(name) {
            return Err(IOError::AlreadyExists);
        }

        let inode = Arc::new(cglue::trait_obj!(RegularINode::<R>::new() as INode));

        let key = self.superblock.inodes.write().insert(inode);

        self.entries.write().insert(name.into(), key);

        Ok(FsINodeRef(key.data().as_ffi()))
    }

    fn flush(&mut self) -> Result<(), IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn seek(&mut self, _mode: SeekMode, _amount: isize) -> Result<usize, IOError> {
        todo!()
    }

    fn next_direntry(&mut self) -> Result<&str, IOError> {
        let range = self.get_range();
        let lock = self.entries.read();
        let Some((next_key, _)) = lock.range::<str, _>(range).next() else {
            return Err(IOError::EOF);
        };
        let next_key = next_key.clone();
        drop(lock);

        Ok(self.current.insert(next_key))
    }
}
