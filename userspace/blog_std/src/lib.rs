#![no_std]

use core::{fmt::Write, mem::MaybeUninit, panic::PanicInfo};

use alloc::string::ToString;
use blog_os_syscalls::SyscallNumber;
use io_error::IOError;
use num_enum::TryFromPrimitive;

use fs::Stat;
pub use path;
use path::Path;
use shared_fs::dirent::{DirEntry, DirEntryHeader};

extern crate alloc;

pub mod file;
pub mod io;
pub mod lock;
pub mod memory;
mod syscalls;

pub mod fs;

fn u64_as_result<E: TryFromPrimitive<Primitive = u64>>(x: u64) -> Result<u64, E> {
    let neg_err = (-(x as i64)) as u64;

    E::try_from_primitive(neg_err).map_or_else(|_| Ok(x), |e| Err(e))
}

pub fn nop(code: u64) {
    unsafe { syscalls::syscall_arg1(SyscallNumber::NOP, code) };
}

pub fn exit(code: u64) -> ! {
    unsafe { syscalls::syscall_arg1(SyscallNumber::EXIT, code) };
    unreachable!()
}

pub fn write(fd: u64, buf: &[u8]) -> Result<u64, IOError> {
    let raw = buf.as_ptr() as u64;
    let len = buf.len() as u64;

    u64_as_result(unsafe { syscalls::syscall_arg3(SyscallNumber::WRITE, len, raw, fd) })
}

pub fn read(fd: u64, buf: &mut [u8]) -> Result<u64, IOError> {
    let raw = buf.as_ptr() as u64;
    let len = buf.len() as u64;

    u64_as_result(unsafe { syscalls::syscall_arg3(SyscallNumber::READ, len, raw, fd) })
}

pub fn brk(offset: i64) -> *mut u8 {
    (unsafe { syscalls::syscall_arg1(SyscallNumber::BRK, offset as u64) }) as *mut u8
}

pub fn yield_syscall() {
    unsafe { syscalls::syscall_arg0(SyscallNumber::YIELD) };
}

pub fn open(path: &Path) -> Result<u64, IOError> {
    let string = path.to_string();
    let bytes = string.as_bytes();
    let raw = bytes.as_ptr() as u64;
    let len = bytes.len() as u64;

    u64_as_result(unsafe { syscalls::syscall_arg2(SyscallNumber::OPEN, len, raw) })
}

pub fn close(fd: u64) -> Result<u64, IOError> {
    u64_as_result(unsafe { syscalls::syscall_arg1(SyscallNumber::CLOSE, fd) })
}

pub fn flush(fd: u64) -> Result<u64, IOError> {
    u64_as_result(unsafe { syscalls::syscall_arg1(SyscallNumber::FLUSH, fd) })
}

pub fn stat(path: &Path) -> Result<Stat, IOError> {
    let string = path.to_string();
    let bytes = string.as_bytes();
    let raw = bytes.as_ptr() as u64;
    let len = bytes.len() as u64;

    let mut stat = MaybeUninit::<Stat>::zeroed();

    let ptr = stat.as_mut_ptr();

    u64_as_result(unsafe { syscalls::syscall_arg3(SyscallNumber::STAT, ptr as u64, len, raw) })?;

    Ok(unsafe { stat.assume_init() })
}

pub fn next_direntry(fd: u64, entry: &mut DirEntry) -> Result<(), IOError> {
    let ptr = core::ptr::from_mut(entry);

    let ptr = ptr as *mut DirEntryHeader;

    u64_as_result(unsafe { syscalls::syscall_arg2(SyscallNumber::NEXT_DIRENTRY, ptr as u64, fd) })?;
    Ok(())
}

/// Closes the fd
pub fn init_driver(fd: u64) -> Result<(), IOError> {
    u64_as_result(unsafe { syscalls::syscall_arg1(SyscallNumber::INIT_DRIVER, fd) })?;
    Ok(())
}

pub fn print(s: &str) {
    let mut buf = s.as_bytes();
    while !buf.is_empty() {
        let bytes = write(1, buf).unwrap() as usize;
        // nop(bytes as u64);
        if let Some(s) = buf.get(bytes..) {
            buf = s;
        } else {
            break;
        }
    }
}

struct StdoutWriter;

impl core::fmt::Write for StdoutWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        print(s);
        Ok(())
    }
}

pub fn print_fmt(args: core::fmt::Arguments) {
    StdoutWriter.write_fmt(args).expect("write");
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print_fmt(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

// Required panic handler
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{info}");
    exit(!0);
}
