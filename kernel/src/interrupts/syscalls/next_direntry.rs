use blog_os_vfs::api::{IOError, file::File};
use x86_64::VirtAddr;

use crate::multitask::get_current_process_info;

fn next_direntry_high_level(fd: u64, buf: &mut [u8]) -> Result<u64, IOError> {
    let file = get_current_process_info()
        .and_then(|pinf| pinf.files().read().get(fd as usize).cloned())
        .ok_or(IOError::NotFound)?;

    // file.write().next_direntry(buf).map(|x| x as u64)
    todo!()
}

pub fn next_direntry(fd: u64, buf: u64, len: u64, _: u64, _: u64, _: u64) -> u64 {
    let buf = unsafe {
        core::slice::from_raw_parts_mut(VirtAddr::new(buf).as_mut_ptr::<u8>(), len as usize)
    };

    next_direntry_high_level(fd, buf).unwrap_or_else(|e| (-(e as i64)) as u64)
}
