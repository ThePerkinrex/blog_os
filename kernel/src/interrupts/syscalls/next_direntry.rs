use blog_os_vfs::api::{IOError, file::File};
use shared_fs::dirent::{DirEntry, DirEntryHeader};
use x86_64::VirtAddr;

use crate::multitask::get_current_process_info;

fn next_direntry_high_level(fd: u64, res: &mut DirEntry) -> Result<u64, IOError> {
    let file = get_current_process_info()
        .and_then(|pinf| pinf.files().read().get(fd as usize).cloned())
        .ok_or(IOError::NotFound)?;

    let mut lock = file.write();
    let name = lock.next_direntry()?;

    let entry_bytes = res.name_buf_mut();

    let name: &[u8] = name.as_bytes();

    let maxlen = entry_bytes.len().min(name.len());

    entry_bytes[..maxlen].copy_from_slice(&name[..maxlen]);

    drop(lock);

    if entry_bytes.len() > maxlen {
        entry_bytes[maxlen] = 0; // NULL termination
    }

    Ok(0)
}

pub fn next_direntry(fd: u64, dirent: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    let ptr = VirtAddr::new_truncate(dirent).as_mut_ptr::<DirEntryHeader>();

    let entry = unsafe { DirEntry::from_thin(ptr) };

    next_direntry_high_level(fd, entry).unwrap_or_else(|e| (-(e as i64)) as u64)
}
