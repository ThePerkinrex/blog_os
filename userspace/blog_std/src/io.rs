use io_error::IOError;

pub type IoResult<T> = Result<T, IOError>;
pub trait Write {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize>;
    fn flush(&mut self) -> IoResult<()>;

    fn write_all(&mut self, mut buf: &[u8]) -> IoResult<()> {
        while !buf.is_empty() {
            let bytes = self.write(buf)?;

            if let Some(s) = buf.get(bytes..) {
                buf = s;
            } else {
                break;
            }
        }
        Ok(())
    }
}

pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize>;
}
