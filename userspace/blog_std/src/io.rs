use alloc::vec::Vec;
use io_error::IOError;

pub type IoResult<T> = Result<T, IOError>;
pub trait Write {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize>;
    fn flush(&mut self) -> IoResult<()>;

    fn write_all(&mut self, mut buf: &[u8]) -> IoResult<usize> {
        let mut wrote = 0;
        while !buf.is_empty() {
            let bytes = self.write(buf)?;
            wrote += bytes;

            if let Some(s) = buf.get(bytes..) {
                buf = s;
            } else {
                break;
            }
        }
        Ok(wrote)
    }
}

pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize>;

    fn read_all(&mut self, mut buf: &mut [u8]) -> IoResult<usize> {
        let mut read = 0;
        while !buf.is_empty() {
            let bytes = self.read(buf)?;

            read += bytes;

            if let Some(s) = buf.get_mut(bytes..) {
                buf = s;
            } else {
                break;
            }
        }

        Ok(read)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        let mut read = 0;
        let mut temp = [0; 32];
        loop {
            let bytes = match self.read(&mut temp) {
                Err(IOError::EOF) => break,
                x => x,
            }?;

            read += bytes;

            buf.extend_from_slice(&temp[..bytes]);
        }

        Ok(read)
    }
}
