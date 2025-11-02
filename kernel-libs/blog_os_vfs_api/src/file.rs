use alloc::{boxed::Box, vec::Vec};
use blog_os_device_api::DeviceId;

use crate::{IOError, inode::FsINodeRef};

pub trait File {
    // TODO standard ops
    fn close(self) -> Result<(), IOError>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IOError>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, IOError>;
    fn readdir(&self) -> Result<Vec<Box<str>>, IOError>;
    fn mkdir(&mut self, name: &str) -> Result<FsINodeRef, IOError>;
    fn mknod(&mut self, name: &str, device: DeviceId) -> Result<FsINodeRef, IOError>;
    fn creat(&mut self, name: &str) -> Result<FsINodeRef, IOError>;
}
