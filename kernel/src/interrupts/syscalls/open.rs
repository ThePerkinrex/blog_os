use alloc::{string::String, sync::Arc};
use blog_os_vfs::api::{IOError, inode::INode, path::PathBuf};
use log::debug;
use spin::lock_api::RwLock;
use x86_64::VirtAddr;

use crate::{fs::VFS, multitask::get_current_process_info, process::OpenFile};

fn open_high_level(path: &str) -> Result<u64, IOError> {
    debug!("Opening: {path}");
    let path = PathBuf::parse(path);
    get_current_process_info()
        .ok_or(IOError::NotFound)
        .and_then(|pinf| {
            let inode = VFS.write().get(&path)?;
            let file = inode.open()?;
            let fd = Arc::new(RwLock::new(OpenFile::new(inode, file)));
            Ok(pinf.files().write().insert(fd) as u64)
        })
        .inspect(|fd| debug!("Opened with fd {fd}"))
}

pub fn open(path: u64, len: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    let buf = String::from_utf8_lossy(unsafe {
        core::slice::from_raw_parts(VirtAddr::new(path).as_ptr::<u8>(), len as usize)
    });

    open_high_level(&buf).unwrap_or_else(|e| (-(e as i64)) as u64)
}
