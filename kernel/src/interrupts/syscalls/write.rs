use alloc::string::String;
use log::{trace, warn};
use x86_64::VirtAddr;

use crate::multitask::{
    change_current_process_info, get_current_task_id,
};

pub fn write(fd: u64, buf: u64, len: u64, _: u64, _: u64, _: u64) -> u64 {
    let task_id = get_current_task_id();
    if fd != 1 {
        warn!("[{task_id:?}] Tried to write an fd different from 1 ({fd})");
    }
    trace!("[{task_id:?}] writing to fd {fd} with buf {buf:x} (len: {len})");
    let buf =
        unsafe { core::slice::from_raw_parts(VirtAddr::new(buf).as_ptr::<u8>(), len as usize) };
    let s = String::from_utf8_lossy(buf);

    change_current_process_info(|pinf| {
        pinf.as_mut().map_or_else(
            || {
                warn!("No process to print {s:?}");
                s.len()
            },
            |pinf| pinf.stdout_mut().write(&s),
        ) as u64
    })
}
