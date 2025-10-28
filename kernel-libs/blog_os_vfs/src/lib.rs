#![no_std]

extern crate alloc;

use alloc::boxed::Box;

use crate::{dentry::DEntryCache, fs::Superblock, inode::FsINodeRef};

pub mod dentry;
pub mod fs;
pub mod inode;

slotmap::new_key_type! {
    pub struct FsIdx;
}

#[derive(Debug, Clone)]
pub struct INodeRef(FsIdx, FsINodeRef);

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

    pub fn add_superblock_box(&mut self, superblock: Box<dyn Superblock>) -> FsIdx {
        self.superblocks.insert(superblock)
    }

    pub fn add_superblock<S: Superblock + 'static>(&mut self, superblock: S) -> FsIdx {
        self.superblocks.insert(Box::new(superblock))
    }
}

impl Default for VFS {
    fn default() -> Self {
        Self::new()
    }
}
