use core::ffi::c_char;

use alloc::boxed::Box;
use io_error::IOError;
use path::Path;
use shared_fs::dirent::DirEntry;
pub use shared_fs::*;

use crate::{close, next_direntry, open};

pub struct DirIter {
    fd: u64,
}

impl DirIter {
    pub fn open(path: &Path) -> Result<Self, IOError> {
        let fd = open(path)?;
        Ok(Self { fd })
    }
}

impl Drop for DirIter {
    fn drop(&mut self) {
        close(self.fd).unwrap();
    }
}

impl Iterator for DirIter {
    type Item = Result<Box<DirEntry>, IOError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut entry = Box::new(DirEntry::<[c_char; 256]>::new_const_cap()) as Box<DirEntry>;
        match next_direntry(self.fd, &mut entry) {
            Err(IOError::EOF) => None,
            x => Some(x.map(|_| entry)),
        }
    }
}
