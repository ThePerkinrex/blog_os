use api_utils::cglue;
use blog_os_device_api::DeviceId;

use crate::{IOError, inode::FsINodeRef};

#[repr(C)]
pub enum SeekMode {
    START,
    CURSOR,
    END,
}

#[cglue::cglue_trait]
pub trait File {
    // TODO standard ops
    fn close(&mut self) -> Result<(), IOError>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IOError>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, IOError>;
    fn seek(&mut self, mode: SeekMode, amount: isize) -> Result<usize, IOError>;
    fn next_direntry(&mut self) -> Result<&str, IOError>;
    fn mkdir(&mut self, name: &str) -> Result<FsINodeRef, IOError>;
    fn mknod(&mut self, name: &str, device: DeviceId) -> Result<FsINodeRef, IOError>;
    fn creat(&mut self, name: &str) -> Result<FsINodeRef, IOError>;
    fn flush(&mut self) -> Result<(), IOError>;
}
