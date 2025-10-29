use alloc::{boxed::Box, vec::Vec};

use crate::IOError;

pub trait File {
    // TODO standard ops
    fn close(self) -> Result<(), IOError>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IOError>;
    fn write(&mut self, buf: &[u8]) -> Result<usize, IOError>;
    fn readdir(&self) -> Result<Vec<Box<str>>, IOError>;
}
