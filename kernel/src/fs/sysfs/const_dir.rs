use core::marker::PhantomData;

use alloc::sync::Arc;
use api_utils::cglue;
use blog_os_vfs::api::{
    file::{File, cglue_file::*},
};
use slotmap::Key;

use crate::fs::sysfs::{INodes, SysFsINode};

pub trait ConstDir<const N: usize>: Send + Sync + 'static {
    const DIR_NAMES: &'static [&'static str; N];

    fn create_dirs(
        inodes: INodes,
    ) -> [SysFsINode; N];
}

#[derive(Clone)]
pub struct ConstDirData<const N: usize, C: ConstDir<N>> {
    dirs: Arc<[SysFsINode; N]>,
    _c: PhantomData<C>,
}

pub struct ConstDirINode<const N: usize, C: ConstDir<N>> {
    data: ConstDirData<N, C>,
}

impl<const N: usize, C: ConstDir<N>> ConstDirINode<N, C> {
    pub fn new(inodes: INodes) -> Self {

        let dirs = C::create_dirs(inodes);

        Self {
            data: ConstDirData {
                dirs: Arc::new(dirs),
                _c: PhantomData,
            },
        }
    }
}

impl<const N: usize, C: ConstDir<N>> blog_os_vfs::api::inode::INode for ConstDirINode<N, C> {
    fn lookup(&self, component: &str) -> Option<blog_os_vfs::api::inode::FsINodeRef> {
        C::DIR_NAMES
            .iter()
            .position(|&n| n == component)
            .and_then(|i| self.data.dirs.get(i))
            .map(|x| blog_os_vfs::api::inode::FsINodeRef(x.data().as_ffi()))
    }

    fn stat(&self) -> Result<shared_fs::Stat, blog_os_vfs::api::IOError> {
        Ok(shared_fs::Stat {
            device: None,
            size: N as u64,
            file_type: shared_fs::FileType::Directory,
        })
    }

    fn open(&self) -> Result<FileBox<'static>, blog_os_vfs::api::IOError> {
        let f = ConstDirFile::<N, C> {
            idx: 0,
            _c: PhantomData,
        };
        Ok(cglue::trait_obj!(f as File))
        // let base: FileBaseBox<'static, ConstDirFile::<N, C>> = From2::from2(f);
        // Ok(base.into_opaque())
    }
}

pub struct ConstDirFile<const N: usize, C: ConstDir<N>> {
    idx: usize,
    _c: PhantomData<C>,
}

impl<const N: usize, C: ConstDir<N>> File for ConstDirFile<N, C> {
    fn close(&mut self) -> Result<(), blog_os_vfs::api::IOError> {
        Ok(())
    }

    fn read(&mut self, _: &mut [u8]) -> Result<usize, blog_os_vfs::api::IOError> {
        Err(blog_os_vfs::api::IOError::OperationNotPermitted)
    }

    fn write(&mut self, _: &[u8]) -> Result<usize, blog_os_vfs::api::IOError> {
        Err(blog_os_vfs::api::IOError::OperationNotPermitted)
    }

    fn seek(
        &mut self,
        _: blog_os_vfs::api::file::SeekMode,
        _: isize,
    ) -> Result<usize, blog_os_vfs::api::IOError> {
        Err(blog_os_vfs::api::IOError::OperationNotPermitted)
    }

    fn next_direntry(&mut self) -> Result<&'static str, blog_os_vfs::api::IOError> {
        if self.idx >= N {
            return Err(blog_os_vfs::api::IOError::EOF);
        }

        let name = C::DIR_NAMES[self.idx];
        self.idx += 1;
        Ok(name)
    }

    fn mkdir(
        &mut self,
        _: &str,
    ) -> Result<blog_os_vfs::api::inode::FsINodeRef, blog_os_vfs::api::IOError> {
        Err(blog_os_vfs::api::IOError::OperationNotPermitted)
    }

    fn mknod(
        &mut self,
        _: &str,
        _: shared_fs::DeviceId,
    ) -> Result<blog_os_vfs::api::inode::FsINodeRef, blog_os_vfs::api::IOError> {
        Err(blog_os_vfs::api::IOError::OperationNotPermitted)
    }

    fn creat(
        &mut self,
        _: &str,
    ) -> Result<blog_os_vfs::api::inode::FsINodeRef, blog_os_vfs::api::IOError> {
        Err(blog_os_vfs::api::IOError::OperationNotPermitted)
    }

    fn flush(&mut self) -> Result<(), blog_os_vfs::api::IOError> {
        Err(blog_os_vfs::api::IOError::OperationNotPermitted)
    }
}

#[macro_export]
macro_rules! const_dir {
    (
        $(#[$meta:meta])*
        $vis:vis struct $Inode:ident {

            dirs = [
                $(
                    {
                        name: $name:literal,
                        inode: $inode_ty:ty
                    }
                ),+ $(,)?
            ];
        }
    ) => {
        paste::paste! {
            #[allow(non_snake_case)]
            $vis mod [<mod_ $Inode>] {
                use super::*;
                pub struct ConstDirData;

                pub const N_NAMES: usize = ${count($name)};

                impl $crate::fs::sysfs::const_dir::ConstDir<N_NAMES> for ConstDirData {
                    const DIR_NAMES: &'static [&'static str; N_NAMES] = &[ $($name,)+ ];

                    fn create_dirs(
                        inodes: $crate::fs::sysfs::INodes,
                    ) -> [$crate::fs::sysfs::SysFsINode; N_NAMES] {
                        use api_utils::cglue;
                        use slotmap::Key;
                        use blog_os_vfs::api::inode::cglue_inode::*;

                        let mut dirs = [$crate::fs::sysfs::SysFsINode::null(); N_NAMES ];
                        let mut idx = 0usize;
                        let mut lock = inodes.write();
                        $(
                            dirs[idx] = lock.insert(alloc::sync::Arc::new(
                                cglue::trait_obj!(
                                    <$inode_ty>::new(inodes.clone()) as INode
                                )
                            ));
                            idx += 1;
                        )+

                        drop(lock);

                        dirs
                    }
                }
            }
            $vis type $Inode = $crate::fs::sysfs::const_dir::ConstDirINode<{[<mod_ $Inode>]::N_NAMES}, [<mod_ $Inode>]::ConstDirData>;
        }

    };
}

// #[macro_export]
// macro_rules! const_dir_inode {
//     (
//         $(#[$meta:meta])*
//         $vis:vis struct $Inode:ident {
//             file $File:ident;

//             dirs = [
//                 $(
//                     {
//                         name: $name:literal,
//                         inode: $inode_ty:ty
//                     }
//                 ),+ $(,)?
//             ];
//         }
//     ) => {
//         use alloc::sync::Arc;

//         const DIR_NAMES: &'static [&'static str] = &[
//             $($name,)+
//         ];

//         #[derive(Clone)]
//         struct Data {
//             dirs: Arc<[$crate::fs::sysfs::SysFsINode; DIR_NAMES.len()]>,
//         }

//         $(#[$meta])*
//         $vis struct $Inode {
//             data: Data,
//         }

//         impl $Inode {
//             pub fn new(inodes: $crate::fs::sysfs::INodes) -> Self {
//                 use api_utils::cglue;
//                 use blog_os_vfs::api::inode::INode;

//                 let mut dirs = [$crate::fs::sysfs::SysFsINode::null();
//                     DIR_NAMES.len()
//                 ];

//                 let mut lock = inodes.write();

//                 let mut idx = 0usize;
//                 $(
//                     dirs[idx] = lock.insert(Arc::new(
//                         cglue::trait_obj!(
//                             <$inode_ty>::new(inodes.clone()) as INode
//                         )
//                     ));
//                     idx += 1;
//                 )+

//                 drop(lock);

//                 Self {
//                     data: Data {
//                         dirs: Arc::new(dirs),
//                     }
//                 }
//             }
//         }

//         impl blog_os_vfs::api::inode::INode for $Inode {
//             fn lookup(
//                 &self,
//                 component: &str
//             ) -> Option<blog_os_vfs::api::inode::FsINodeRef> {
//                 DIR_NAMES
//                     .iter()
//                     .position(|&n| n == component)
//                     .and_then(|i| self.data.dirs.get(i))
//                     .map(|x| blog_os_vfs::api::inode::FsINodeRef(x.data().as_ffi()))
//             }

//             fn stat(
//                 &self
//             ) -> Result<shared_fs::Stat, blog_os_vfs::api::IOError> {
//                 Ok(shared_fs::Stat {
//                     device: None,
//                     size: DIR_NAMES.len() as u64,
//                     file_type: shared_fs::FileType::Directory,
//                 })
//             }

//             fn open(
//                 &self
//             ) -> Result<
//                 blog_os_vfs::api::file::FileBox<'static>,
//                 blog_os_vfs::api::IOError
//             > {
//                 Ok(api_utils::cglue::trait_obj!(
//                     $File { idx: 0 } as blog_os_vfs::api::file::File
//                 ))
//             }
//         }

//         struct $File {
//             idx: usize,
//         }

//         impl blog_os_vfs::api::file::File for $File {
//             fn close(&mut self) -> Result<(), blog_os_vfs::api::IOError> {
//                 Ok(())
//             }

//             fn read(
//                 &mut self,
//                 _: &mut [u8]
//             ) -> Result<usize, blog_os_vfs::api::IOError> {
//                 Err(blog_os_vfs::api::IOError::OperationNotPermitted)
//             }

//             fn write(
//                 &mut self,
//                 _: &[u8]
//             ) -> Result<usize, blog_os_vfs::api::IOError> {
//                 Err(blog_os_vfs::api::IOError::OperationNotPermitted)
//             }

//             fn seek(
//                 &mut self,
//                 _: blog_os_vfs::api::file::SeekMode,
//                 _: isize
//             ) -> Result<usize, blog_os_vfs::api::IOError> {
//                 Err(blog_os_vfs::api::IOError::OperationNotPermitted)
//             }

//             fn next_direntry(
//                 &mut self
//             ) -> Result<&'static str, blog_os_vfs::api::IOError> {
//                 if self.idx >= DIR_NAMES.len() {
//                     return Err(blog_os_vfs::api::IOError::EOF);
//                 }

//                 let name = DIR_NAMES[self.idx];
//                 self.idx += 1;
//                 Ok(name)
//             }

//             fn mkdir(
//                 &mut self,
//                 _: &str
//             ) -> Result<
//                 blog_os_vfs::api::inode::FsINodeRef,
//                 blog_os_vfs::api::IOError
//             > {
//                 Err(blog_os_vfs::api::IOError::OperationNotPermitted)
//             }

//             fn mknod(
//                 &mut self,
//                 _: &str,
//                 _: shared_fs::DeviceId
//             ) -> Result<
//                 blog_os_vfs::api::inode::FsINodeRef,
//                 blog_os_vfs::api::IOError
//             > {
//                 Err(blog_os_vfs::api::IOError::OperationNotPermitted)
//             }

//             fn creat(
//                 &mut self,
//                 _: &str
//             ) -> Result<
//                 blog_os_vfs::api::inode::FsINodeRef,
//                 blog_os_vfs::api::IOError
//             > {
//                 Err(blog_os_vfs::api::IOError::OperationNotPermitted)
//             }

//             fn flush(
//                 &mut self
//             ) -> Result<(), blog_os_vfs::api::IOError> {
//                 Err(blog_os_vfs::api::IOError::OperationNotPermitted)
//             }
//         }
//     };
// }
