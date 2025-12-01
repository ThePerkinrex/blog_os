use core::marker::PhantomData;

use alloc::sync::Arc;
use api_utils::cglue::{self, arc::CArc};
use blog_os_vfs_api::{
    fs::Superblock,
    inode::{FsINodeRef, cglue_inode::*},
};
use lock_api::{RawRwLock, RwLock};
use slotmap::KeyData;

use crate::inode::directory::DirectoryINode;

slotmap::new_key_type! {pub(crate) struct INodeKey;}

pub(crate) struct SharedSuperblockData<R: RawRwLock + Send + Sync + 'static> {
    pub(crate) inodes: RwLock<R, slotmap::SlotMap<INodeKey, Arc<INodeBox<'static>>>>,
}

pub struct RamFSSuperblock<R: RawRwLock + Send + Sync + 'static> {
    root_inode: INodeKey,
    data: Arc<SharedSuperblockData<R>>,
    _marker: PhantomData<R>,
}

impl<R: RawRwLock + Send + Sync> RamFSSuperblock<R> {
    fn new() -> Self {
        let data = Arc::new(SharedSuperblockData {
            inodes: RwLock::new(slotmap::SlotMap::with_key()),
        });
        let root_inode =
            data.inodes
                .write()
                .insert(Arc::new(cglue::trait_obj!(
                    DirectoryINode::<R>::new(data.clone()) as INode
                )));
        Self {
            root_inode,
            data,
            _marker: PhantomData,
        }
    }
}

impl<R: RawRwLock + Send + Sync> Default for RamFSSuperblock<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: RawRwLock + Send + Sync> Superblock for RamFSSuperblock<R> {
    fn get_root_inode_ref(&self) -> FsINodeRef {
        FsINodeRef(self.root_inode.0.as_ffi())
    }

    fn get_inode(&self, inode: FsINodeRef) -> CArc<INodeBox<'static>> {
        let key = INodeKey::from(KeyData::from_ffi(inode.0));

        self.data.inodes.read().get(key).cloned().into()
    }

    fn unmount(self) {
        drop(self);
    }
}
