use blog_os_vfs::api::{IOError, file::File};

use crate::multitask::get_current_process_info;

fn close_high_level(fd: u64) -> Result<u64, IOError> {
    let file = get_current_process_info()
        .and_then(|pinf| pinf.files().write().remove(fd as usize))
        .ok_or(IOError::NotFound)?;

    file.write().close()?;
    Ok(0)
}

pub fn close(fd: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    // let buf = unsafe {
    //     core::slice::from_raw_parts_mut(VirtAddr::new(buf).as_mut_ptr::<u8>(), len as usize)
    // };

    close_high_level(fd).unwrap_or_else(|e| (-(e as i64)) as u64)
}
