use blog_os_vfs::api::{IOError, file::File};
use log::debug;
use x86_64::VirtAddr;

use crate::multitask::get_current_process_info;

fn write_high_level(fd: u64, buf: &[u8]) -> Result<u64, IOError> {
    let file = get_current_process_info()
        .and_then(|pinf| pinf.files().read().get(fd as usize).cloned())
        .ok_or(IOError::NotFound)?;
    debug!("Loaded file for writing");

    file.write().write(buf).map(|x| x as u64)
}

pub fn write(fd: u64, buf: u64, len: u64, _: u64, _: u64, _: u64) -> u64 {
    debug!("Loading buffer for writing (@ 0x{buf:x} with len {len})");
    let buf =
        unsafe { core::slice::from_raw_parts(VirtAddr::new(buf).as_ptr::<u8>(), len as usize) };
    debug!("Loaded buffer for writing");

    write_high_level(fd, buf).unwrap_or_else(|e| (-(e as i64)) as u64)
}
