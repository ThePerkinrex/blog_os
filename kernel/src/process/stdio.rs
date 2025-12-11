use alloc::{boxed::Box, collections::vec_deque::VecDeque, string::String, sync::Arc, vec::Vec};
use blog_os_device::api::DeviceId;
use blog_os_vfs::api::{
    IOError,
    file::{File, SeekMode},
    inode::FsINodeRef,
};
use log::{error, info};
use spin::lock_api::RwLock;


#[derive(Debug, Default)]
pub struct StdInData {
    buffer: Vec<u8>,
    eof: bool
}

impl StdInData {
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }
    
    pub const fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }
}

#[derive(Debug)]
pub struct StdIn {
    data: Arc<RwLock<StdInData>>
}

impl StdIn {
    pub const fn new(data: Arc<RwLock<StdInData>>) -> Self {
        Self { data }
    }
}



impl File for StdIn {
    fn close(&mut self) -> Result<(), IOError> {
        self.data.write().eof = true;
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, IOError> {
        if self.data.read().eof {
            return Err(IOError::EOF)
        }

        let mut lock = self.data.write();
        
        let bytes = buf.len().min(lock.buffer.len());

        let next = lock.buffer.split_off(bytes);
        let read = core::mem::replace(&mut lock.buffer, next);

        buf[..bytes].copy_from_slice(&read);

        Ok(bytes)
    }

    fn write(&mut self, _buf: &[u8]) -> Result<usize, IOError> {
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

    fn flush(&mut self) -> Result<(), IOError> {
        Ok(())
    }

    fn seek(&mut self, mode: SeekMode, amount: isize) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn next_direntry(&mut self) -> Result<&str, IOError> {
        Err(IOError::OperationNotPermitted)
    }
}

pub struct Out {
    print: fn(&str),
    buf: String,
}

impl File for Out {
    fn close(&mut self) -> Result<(), IOError> {
        Ok(())
    }

    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, IOError> {
        let data = String::from_utf8_lossy(buf);

        let combined = core::mem::take(&mut self.buf) + &data;

        let lines = combined.split_inclusive('\n');

        let mut sum = 0;
        for line in lines {
            sum += line.len();
            if line.ends_with('\n') {
                (self.print)(line.trim_end_matches('\n'));
                // info!("[STDOUT] {}", line.trim_end_matches('\n'));
            } else {
                self.buf += line;
            }
        }

        Ok(sum)
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
        (self.print)(&self.buf);

        self.buf.clear();
        Ok(())
    }

    fn seek(&mut self, mode: SeekMode, amount: isize) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn next_direntry(&mut self) -> Result<&str, IOError> {
        Err(IOError::OperationNotPermitted)
    }
}

pub fn stdout() -> Out {
    Out {
        print: |s| info!("[STDOUT] {s}"),
        buf: String::new(),
    }
}

pub fn stderr() -> Out {
    Out {
        print: |s| error!("[STDERR] {s}"),
        buf: String::new(),
    }
}
