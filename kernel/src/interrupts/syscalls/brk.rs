use log::debug;

use crate::multitask::get_current_process_info;

pub fn brk(offset: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    debug!("BRK SYSCALL ({offset})");
    let offset = offset as i64;
    let Some(pinf) = get_current_process_info() else {
        return 0;
    };

    let prog = pinf.program();

    prog.heap()
        .lock()
        .change_brk(prog.stack(), offset)
        .map_or(-1i64 as u64, |addr| addr.as_u64())
}
