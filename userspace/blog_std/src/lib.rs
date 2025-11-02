#![no_std]

use core::{fmt::Write, panic::PanicInfo};

mod syscalls;

pub fn nop(code: u64) {
    unsafe { syscalls::syscall_arg1(0x0, code) };
}

pub fn exit(code: u64) -> ! {
    unsafe { syscalls::syscall_arg1(0x1, code) };
    unreachable!()
}

pub fn write(fd: u64, buf: &[u8]) -> u64 {
    let raw = buf.as_ptr() as u64;
    let len = buf.len() as u64;

    unsafe { syscalls::syscall_arg3(0x2, len, raw, fd) }
}

pub fn print(s: &str) {
    let mut buf = s.as_bytes();
    while !buf.is_empty() {
        let bytes = write(1, buf) as usize;
        buf = &buf[bytes..]
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
