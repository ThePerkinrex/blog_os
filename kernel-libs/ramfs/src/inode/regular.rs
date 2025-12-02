use alloc::{sync::Arc, vec::Vec};
use api_utils::cglue;
use blog_os_device_api::DeviceId;
use blog_os_vfs_api::{
    IOError,
    file::{File, SeekMode, cglue_file::*},
    inode::{FsINodeRef, INode},
};
use log::debug;
use shared_fs::{FileType, Stat};

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
            file_type: FileType::RegularFile,
        })
    }

    fn open(&self) -> Result<FileBox<'static>, IOError> {
        Ok(cglue::trait_obj!(RegularFile::<R> {
            data: self.data.clone(),
            cursor: 0
        } as File))
    }
}

pub struct RegularFile<R: RawRwLock + Send + Sync + 'static> {
    data: Arc<RwLock<R, Vec<u8>>>,
    cursor: usize,
}

impl<R: RawRwLock + Send + Sync> File for RegularFile<R> {
    fn close(&mut self) -> Result<(), IOError> {
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IOError> {
        let lock = self.data.read();

        if self.cursor >= lock.len() {
            return Err(IOError::EOF);
        }

        let self_data = &lock[self.cursor..];

        let bytes = self_data.len().min(buf.len());

        debug!(
            "Reading {bytes} bytes from the cursor: {} (len: {})",
            self.cursor,
            lock.len()
        );

        buf[..bytes].copy_from_slice(&self_data[..bytes]);

        drop(lock);

        self.cursor += bytes;

        Ok(bytes)
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, IOError> {
        let mut lock = self.data.write();

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

    fn mkdir(&mut self, _name: &str) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn mknod(&mut self, _name: &str, _device: DeviceId) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn creat(&mut self, _name: &str) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn flush(&mut self) -> Result<(), IOError> {
        // TODO

        Ok(())
    }

    fn seek(&mut self, mode: SeekMode, amount: isize) -> Result<usize, IOError> {
        todo!()
    }

    fn next_direntry(&mut self) -> Result<&str, IOError> {
        Err(IOError::OperationNotPermitted)
    }
}
