use alloc::string::String;
use log::{info, warn};
use x86_64::VirtAddr;

pub fn write(fd: u64, buf: u64, len: u64, _: u64, _: u64, _: u64) -> u64 {
    if fd != 1 {
        warn!("Tried to write an fd different from 1 ({fd})");
    }
    let buf =
        unsafe { core::slice::from_raw_parts(VirtAddr::new(buf).as_ptr::<u8>(), len as usize) };
    let s = String::from_utf8_lossy(buf);
    for l in s.lines() {
        info!("[STDOUT] {l}");
    }
    len
}
