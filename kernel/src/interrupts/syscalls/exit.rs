use log::debug;

use crate::{multitask::change_current_process_info, process::ProcessStatus};

pub fn exit(code: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    debug!("EXIT SYSCALL ({code})");
    change_current_process_info(|p| {
        let pinf = p.as_mut().unwrap(); // Process info must be there if a syscall was made.
        *pinf.status_mut() = ProcessStatus::Ending(code)
    });
    0
}
