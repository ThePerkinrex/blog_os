use alloc::string::String;
use blog_os_vfs::api::{IOError, inode::INode, path::PathBuf};
use shared_fs::Stat;
use x86_64::VirtAddr;

use crate::fs::VFS;

fn stat_high_level(path: &str, stat: &mut Stat) -> Result<u64, IOError> {
    let path = PathBuf::parse(path);

    *stat = VFS.write().get(&path)?.stat()?;

    Ok(0)
}

pub fn stat(path: u64, len: u64, stat: u64, _: u64, _: u64, _: u64) -> u64 {
    let buf = String::from_utf8_lossy(unsafe {
        core::slice::from_raw_parts(VirtAddr::new(path).as_ptr::<u8>(), len as usize)
    });

    let stat = unsafe { VirtAddr::new_truncate(stat).as_mut_ptr::<Stat>().as_mut() }.unwrap();

    stat_high_level(&buf, stat).unwrap_or_else(|e| (-(e as i64)) as u64)
}
