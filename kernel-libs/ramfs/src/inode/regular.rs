use alloc::{boxed::Box, sync::Arc, vec::Vec};
use api_utils::cglue;
use blog_os_device::api::DeviceId;
use blog_os_vfs::api::{
    IOError,
    file::{File, cglue_file::*},
    inode::{FsINodeRef, INode},
    stat::Stat,
};
use lock_api::{RawRwLock, RwLock};

pub struct RegularINode<R: RawRwLock + Send + Sync + 'static> {
    data: Arc<RwLock<R, Vec<u8>>>,
}

impl<R: RawRwLock + Send + Sync> RegularINode<R> {
    pub(crate) fn new() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl<R: RawRwLock + Send + Sync> INode for RegularINode<R> {
    fn lookup(&self, _: &str) -> Option<FsINodeRef> {
        None
    }

    fn stat(&self) -> Result<Stat, IOError> {
        Ok(Stat {
            device: None,
            size: self.data.read().len() as u64,
            file_type: blog_os_vfs::api::stat::FileType::RegularFile,
        })
    }

    fn open(&self) -> Result<FileBox<'_>, IOError> {
        Ok(cglue::trait_obj!(RegularFile::<'_, R> {
            inode: self,
            cursor: 0
        } as File))
    }
}

pub struct RegularFile<'a, R: RawRwLock + Send + Sync + 'static> {
    inode: &'a RegularINode<R>,
    cursor: usize,
}

impl<'a, R: RawRwLock + Send + Sync> File for RegularFile<'a, R> {
    fn close(self) -> Result<(), IOError> {
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IOError> {
        let lock = self.inode.data.read();

        let self_data = &lock[self.cursor..];

        let bytes = self_data.len().min(buf.len());

        buf[..bytes].copy_from_slice(&self_data[..bytes]);

        drop(lock);

        self.cursor += bytes;

        Ok(bytes)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, IOError> {
        let mut lock = self.inode.data.write();

        let self_data = &mut lock[self.cursor..];

        let bytes = if self_data.is_empty() {
            lock.extend_from_slice(data);
            data.len()
        } else {
            let bytes = self_data.len().min(data.len());

            self_data[..bytes].copy_from_slice(&data[..bytes]);
            bytes
        };

        drop(lock);

        self.cursor += bytes;

        Ok(bytes)
    }

    fn readdir(&self) -> Result<Vec<Box<str>>, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn mkdir(&mut self, _name: &str) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn mknod(&mut self, _name: &str, _device: DeviceId) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn creat(&mut self, _name: &str) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }
}
