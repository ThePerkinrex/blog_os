use blog_os_vfs::api::file::File;
use log::debug;

use crate::{multitask::change_current_process_info, process::ProcessStatus};

pub fn exit(code: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    debug!("EXIT SYSCALL ({code})");
    change_current_process_info(|p| {
        let pinf = p.as_mut().unwrap(); // Process info must be there if a syscall was made.
        for (_fd, file) in pinf.files().read().iter() {
            let mut lock = file.write();

            lock.flush().unwrap();
            lock.close().unwrap();

            drop(lock);
        }
        *pinf.status_mut() = ProcessStatus::Ending(code)
    });
    0
}
