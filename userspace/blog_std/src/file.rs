use io_error::IOError;
use path::Path;

use crate::{
    close, flush,
    io::{Read, Write},
    open, read, write,
};

pub struct File {
    fd: u64,
}

impl File {
    pub fn open(path: &Path) -> Result<Self, IOError> {
        let fd = open(path)?;

        Ok(Self { fd })
    }

    /// # Safety
    /// The provided fd must be valid
    pub const unsafe fn from_fd(fd: u64) -> Self {
        Self { fd }
    }

    pub fn close(self) {
        drop(self)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        close(self.fd).unwrap();
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> crate::io::IoResult<usize> {
        read(self.fd, buf).map(|x| x as usize)
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> crate::io::IoResult<usize> {
        write(self.fd, buf).map(|x| x as usize)
    }

    fn flush(&mut self) -> crate::io::IoResult<()> {
        flush(self.fd)?;
        Ok(())
    }
}
