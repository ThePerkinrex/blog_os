#![no_std]

extern crate alloc;
pub use blog_os_vfs_api as api;

use alloc::{borrow::ToOwned, boxed::Box};

use crate::{
    api::fs::Superblock,
    api::inode::FsINodeRef,
    api::path::{Path, PathBuf},
    dentry::{DEntry, DEntryCache},
};

pub mod dentry;

slotmap::new_key_type! {
    pub struct FsIdx;
}

#[derive(Debug, Clone, Copy)]
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
            Some(dentry.inode)
        } else {
            let remaining = path
                .relative(greatest)
                .expect("greatest is a prefix of path");
            let mut current = greatest.to_owned();
            let INodeRef(fs, mut inode_ref) = dentry.inode;
            let superblock = &self.superblocks[fs];

            for (i, c) in remaining.components().enumerate() {
                let inode = superblock.get_inode(inode_ref)?;
                if i < remaining.len() - 1 {
                    inode_ref = inode.lookup(c)?;
                    current
                        .push_component(c)
                        .expect("component doesnt contain slash");
                    self.dentry_cache.add_cached(
                        current.clone(),
                        DEntry {
                            inode: INodeRef(fs, inode_ref),
                        },
                    );
                }
            }

            Some(INodeRef(fs, inode_ref))
        }
    }
}

impl Default for VFS {
    fn default() -> Self {
        Self::new()
    }
}
