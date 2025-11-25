#![no_std]

extern crate alloc;
pub use blog_os_vfs_api as api;
use blog_os_vfs_api::{
    cglue::arc::CArcSome,
    fs::{Filesystem, cglue_filesystem::FilesystemBox, cglue_superblock::SuperblockBox},
    inode::{INode, cglue_inode::INodeBox},
    path::ffi::pathbuf_into_ffi_ref,
};

use alloc::{borrow::ToOwned, collections::btree_map::BTreeMap, string::String};

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
    filesystems: BTreeMap<String, FilesystemBox<'static>>,
    superblocks: slotmap::SlotMap<FsIdx, SuperblockBox<'static>>,
    dentry_cache: DEntryCache,
}

#[derive(Debug)]
pub struct AlreadyRegisteredError;

impl VFS {
    pub fn new() -> Self {
        Self {
            filesystems: Default::default(),
            superblocks: Default::default(),
            dentry_cache: DEntryCache::new(),
        }
    }

    pub fn register_fs(
        &mut self,
        fs: FilesystemBox<'static>,
    ) -> Result<(), AlreadyRegisteredError> {
        let key = fs.name();
        if self.filesystems.contains_key(key) {
            return Err(AlreadyRegisteredError);
        }

        self.filesystems.insert(key.into(), fs);
        Ok(())
    }

    pub fn unregister_fs(&mut self, fs: &str) -> Option<FilesystemBox<'static>> {
        self.filesystems.remove(fs)
    }

    fn mount_fs(
        superblocks: &mut slotmap::SlotMap<FsIdx, SuperblockBox<'static>>,
        dentry_cache: &mut DEntryCache,
        path: PathBuf,
        dev: Option<&PathBuf>,
        fs: &FilesystemBox<'static>,
    ) -> Option<FsIdx> {
        let superblock = fs.mount(dev.map(pathbuf_into_ffi_ref))?;

        let root_inode = superblock.get_root_inode_ref();
        let fs = superblocks.insert(superblock);
        dentry_cache.add_mountpoint(
            path,
            DEntry {
                inode: INodeRef(fs, root_inode),
            },
        );
        Some(fs)
    }

    pub fn mount_type(
        &mut self,
        path: PathBuf,
        dev: Option<&PathBuf>,
        fs_type: &str,
    ) -> Option<FsIdx> {
        let fs = self.filesystems.get(fs_type)?;

        Self::mount_fs(&mut self.superblocks, &mut self.dentry_cache, path, dev, fs)
    }

    pub fn mount(&mut self, path: PathBuf, dev: Option<&PathBuf>) -> Option<FsIdx> {
        self.filesystems.values().find_map(|fs| {
            Self::mount_fs(
                &mut self.superblocks,
                &mut self.dentry_cache,
                path.clone(),
                dev,
                fs,
            )
        })
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
                let inode = superblock.get_inode(inode_ref).transpose()?;
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

    pub fn get_inode(&self, inode_ref: INodeRef) -> Option<CArcSome<INodeBox<'static>>> {
        let fs = self.superblocks.get(inode_ref.0)?;

        fs.get_inode(inode_ref.1).into()
    }
}

impl Default for VFS {
    fn default() -> Self {
        Self::new()
    }
}
