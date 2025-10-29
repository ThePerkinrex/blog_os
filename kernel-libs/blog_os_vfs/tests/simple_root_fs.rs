use blog_os_vfs::{
    VFS,
    block::{Block, FsBlockRef},
    fs::Superblock,
    inode::{FsINodeRef, INode},
    path::PathBuf,
};
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

        fn get_block(&self, _: FsBlockRef) -> Option<&Block> {
            todo!()
        }

        fn get_mut_block(&mut self, _: FsBlockRef) -> Option<&mut Block> {
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
    let root = PathBuf::try_from_iter([""]).expect("A correct path");
    let fs = vfs.mount(root.clone(), common::fs::example_fs_empty_files());

    let inode = vfs.get_ref(&root);
    assert!(inode.is_some());
    let inode = inode.unwrap();

    assert_eq!(fs, inode.fs());
    assert_eq!(0, inode.inode().0);

    let sh_path = PathBuf::try_from_iter(["", "bin", "sh"]).expect("A correct path");
    let sh = vfs.get_ref(&sh_path).expect("An inode");
}
