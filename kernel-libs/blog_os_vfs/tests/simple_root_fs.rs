use std::ptr::NonNull;

use blog_os_vfs::{VFS, api::fs::Superblock, api::inode::FsINodeRef, api::path::PathBuf};
use blog_os_vfs_api::{
    cglue::{self, arc::CArc},
    fs::{Filesystem, cglue_filesystem::*, cglue_superblock::*},
    inode::cglue_inode::INodeBox,
    path::ffi::PathBufOpaqueRef,
};
use kernel_utils::try_from_iterator::TryFromIterator;

use crate::common::fs::CustomFs;

mod common;

#[test]
pub fn simple_root_fs() {
    struct Super;

    impl Superblock for Super {
        fn get_root_inode_ref(&self) -> FsINodeRef {
            FsINodeRef(1)
        }

        fn get_inode(&self, _: FsINodeRef) -> CArc<INodeBox<'static>> {
            todo!()
        }

        fn unmount(self) {
            todo!()
        }
    }

    struct Fs;

    impl Filesystem for Fs {
        fn name(&self) -> &str {
            "fs"
        }

        fn mount(
            &self,
            device: Option<NonNull<PathBufOpaqueRef>>,
        ) -> Option<SuperblockBox<'static>> {
            if device.is_some() {
                None
            } else {
                Some(cglue::trait_obj!(Super as Superblock))
            }
        }
    }

    let mut vfs = VFS::new();
    let root: PathBuf = TryFromIterator::try_from_iter([""]).expect("A correct path");
    vfs.register_fs(cglue::trait_obj!(Fs as Filesystem))
        .unwrap();
    let fs = vfs.mount(root.clone(), None);

    let inode = vfs.get_ref(&root);
    assert!(inode.is_some());
    let inode = inode.unwrap();

    assert_eq!(fs, Some(inode.fs()));
    assert_eq!(1, inode.inode().0);
}

#[test]
pub fn tree_root_fs() {
    let mut vfs = VFS::new();
    let root = PathBuf::root();

    vfs.register_fs(cglue::trait_obj!(CustomFs as Filesystem))
        .unwrap();

    let fs = vfs.mount(root.clone(), None);

    let inode = vfs.get_ref(&root);
    assert!(inode.is_some());
    let inode = inode.unwrap();

    assert_eq!(fs, Some(inode.fs()));
    assert_eq!(0, inode.inode().0);

    let sh_path = PathBuf::parse("/bin/sh");
    let sh = vfs.get_ref(&sh_path).expect("An inode");

    assert_eq!(fs, Some(sh.fs()));
    assert!(sh.inode().0 > 0);
}
