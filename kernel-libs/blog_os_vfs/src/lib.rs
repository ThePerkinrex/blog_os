#![no_std]

extern crate alloc;

use alloc::boxed::Box;

use crate::{
    dentry::{DEntry, DEntryCache},
    fs::Superblock,
    inode::FsINodeRef,
    path::{Path, PathBuf},
};

pub mod dentry;
pub mod fs;
pub mod inode;
pub mod path;

slotmap::new_key_type! {
    pub struct FsIdx;
}

#[derive(Debug, Clone)]
pub struct INodeRef(FsIdx, FsINodeRef);

impl INodeRef {
    pub const fn fs(&self) -> FsIdx {
        self.0
    }

    pub const fn inode(&self) -> FsINodeRef {
        self.1
    }
}

pub struct VFS {
    superblocks: slotmap::SlotMap<FsIdx, Box<dyn Superblock>>,
    dentry_cache: DEntryCache,
}

impl VFS {
    pub fn new() -> Self {
        Self {
            superblocks: Default::default(),
            dentry_cache: DEntryCache::new(),
        }
    }

    pub fn mount_box(&mut self, path: PathBuf, superblock: Box<dyn Superblock>) -> FsIdx {
        let root_inode = superblock.get_root_inode_ref();
        let fs = self.superblocks.insert(superblock);
        self.dentry_cache.add_mountpoint(
            path,
            DEntry {
                inode: INodeRef(fs, root_inode),
            },
        );
        fs
    }

    pub fn mount<S: Superblock + 'static>(&mut self, path: PathBuf, superblock: S) -> FsIdx {
        self.mount_box(path, Box::new(superblock))
    }

    pub fn get_ref(&mut self, path: &Path) -> Option<INodeRef> {
        let (dentry, greatest) = self.dentry_cache.find_greatest(path)?;
        if greatest == path {
            Some(dentry.inode.clone())
        } else {
            todo!("Recurse on the fs")
        }
    }
}

impl Default for VFS {
    fn default() -> Self {
        Self::new()
    }
}
