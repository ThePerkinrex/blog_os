use blog_os_vfs::{
    VFS,
    api::fs::Superblock,
    api::inode::{FsINodeRef, INode},
    api::path::PathBuf,
};
use blog_os_vfs_api::{IOError, file::File};
use kernel_utils::try_from_iterator::TryFromIterator;

mod common;

#[test]
pub fn simple_root_fs() {
    struct Super;

    impl Superblock for Super {
        fn get_root_inode_ref(&self) -> FsINodeRef {
            FsINodeRef(1)
        }

        fn get_inode(&self, _: FsINodeRef) -> Option<&dyn INode> {
            todo!()
        }

        fn open(&mut self, _: FsINodeRef) -> Result<Box<dyn File>, IOError> {
            todo!()
        }
    }

    let mut vfs = VFS::new();
    let root: PathBuf = TryFromIterator::try_from_iter([""]).expect("A correct path");
    let fs = vfs.mount(root.clone(), Super);

    let inode = vfs.get_ref(&root);
    assert!(inode.is_some());
    let inode = inode.unwrap();

    assert_eq!(fs, inode.fs());
    assert_eq!(1, inode.inode().0);
}

#[test]
pub fn tree_root_fs() {
    let mut vfs = VFS::new();
    let root = PathBuf::root();
    let fs = vfs.mount(root.clone(), common::fs::example_fs_empty_files());

    let inode = vfs.get_ref(&root);
    assert!(inode.is_some());
    let inode = inode.unwrap();

    assert_eq!(fs, inode.fs());
    assert_eq!(0, inode.inode().0);

    let sh_path = PathBuf::parse("/bin/sh");
    let sh = vfs.get_ref(&sh_path).expect("An inode");

    assert_eq!(fs, sh.fs());
    assert!(sh.inode().0 > 0);
}
