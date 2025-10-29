use blog_os_vfs::{
    VFS,
    fs::Superblock,
    inode::{FsINodeRef, INode},
    path::PathBuf,
};
use kernel_utils::try_from_iterator::TryFromIterator;

struct Super;

impl Superblock for Super {
    fn get_root_inode_ref(&self) -> FsINodeRef {
        FsINodeRef(1)
    }

    fn get_inode(&self, _: FsINodeRef) -> &dyn INode {
        todo!()
    }
}

#[test]
pub fn simple_root_fs() {
    let mut vfs = VFS::new();
    let root: PathBuf = TryFromIterator::try_from_iter([""]).expect("A correct path");
    let fs = vfs.mount(root.clone(), Super);

    let inode = vfs.get_ref(&root);
    assert!(inode.is_some());
    let inode = inode.unwrap();

    assert_eq!(fs, inode.fs());
    assert_eq!(1, inode.inode().0);
}
