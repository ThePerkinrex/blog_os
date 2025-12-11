use alloc::sync::Arc;
use api_utils::cglue;
use blog_os_vfs::api::{
    IOError,
    file::{File, SeekMode, cglue_file::*},
    inode::{FsINodeRef, INode},
};
use num_enum::TryFromPrimitive;
use shared_fs::{DeviceId, Stat};
use slotmap::Key;

use crate::fs::sysfs::{INodes, SysFsINode};

#[derive(Clone)]
struct DeviceData {
    inodes: INodes,
}

pub struct DevicesINode {
    data: DeviceData,
}

impl DevicesINode {
    pub fn new(inodes: INodes) -> Self {
        // let mut dirs = [SysFsINode::null(); Dirs::Max as usize];
        // let mut lock = inodes.write();

        // todo!("Write dirs inodes");

        // drop(lock);
        let data = DeviceData { inodes };

        Self { data }
    }
}

impl INode for DevicesINode {
    fn lookup(&self, component: &str) -> Option<FsINodeRef> {
        None
    }

    fn stat(&self) -> Result<Stat, IOError> {
        Ok(Stat {
            device: None,
            size: 0,
            file_type: shared_fs::FileType::Directory,
        })
    }

    fn open(&self) -> Result<FileBox<'static>, IOError> {
        Ok(cglue::trait_obj!(DevicesFile { idx: 0 } as File))
    }
}

struct DevicesFile {
    idx: u8,
}

impl File for DevicesFile {
    fn close(&mut self) -> Result<(), IOError> {
        Ok(())
    }

    fn read(&mut self, _: &mut [u8]) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn write(&mut self, _: &[u8]) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn seek(&mut self, _: SeekMode, _: isize) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn next_direntry(&mut self) -> Result<&str, IOError> {
        Err(IOError::EOF)
    }

    fn mkdir(&mut self, _: &str) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn mknod(&mut self, _: &str, _: DeviceId) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn creat(&mut self, _: &str) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn flush(&mut self) -> Result<(), IOError> {
        Err(IOError::OperationNotPermitted)
    }
}
