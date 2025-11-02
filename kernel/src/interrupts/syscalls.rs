use alloc::sync::Arc;
use log::{debug, info, warn};

use crate::{
    multitask::{
        change_current_process_info, get_current_process_info, task_exit, task_switch_safe,
    },
    process::ProcessStatus,
};

type SyscallHandler = fn(u64, u64, u64, u64, u64, u64) -> u64;

const SYSCALL_HANDLERS: &[SyscallHandler] = &[nop, exit];

fn nop(arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64, arg6: u64) -> u64 {
    debug!("NOP SYSCALL ({arg1}, {arg2}, {arg3}, {arg4}, {arg5}, {arg6})");
    0
}

fn exit(code: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    debug!("EXIT SYSCALL ({code})");
    change_current_process_info(|p| {
        let pinf = p.as_mut().unwrap(); // Process info must be there if a syscall was made.
        *pinf.status_mut() = ProcessStatus::Ending(code)
    });
    0
}

pub fn syscall_handle(
    code: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
) -> u64 {
    if code < SYSCALL_HANDLERS.len() as u64 {
        SYSCALL_HANDLERS[code as usize](arg1, arg2, arg3, arg4, arg5, arg6)
    } else {
        warn!("Unknown syscall {code} ({arg1}, {arg2}, {arg3}, {arg4}, {arg5}, {arg6})");
        u64::MAX
    }
}

pub extern "C" fn syscall_tail() {
    debug!("SYSCALL TAIL");

    let current_pinf = get_current_process_info().unwrap(); // Process info must be there if a syscall was made.

    if let ProcessStatus::Ending(code) = current_pinf.status() {
        info!("Process ending with code: {code}");
        change_current_process_info(|p| p.take()); // This process is no longer associated with the task
        let program = current_pinf.program().clone();
        drop(current_pinf);

        debug!("Strong count: {}", Arc::strong_count(&program));
        debug!("Weak count: {}", Arc::weak_count(&program));

        let program = Arc::into_inner(program).expect("No more than one ref");
        unsafe { program.unload() };

        x86_64::instructions::interrupts::enable();
        task_exit();
    }

    drop(current_pinf);
    x86_64::instructions::interrupts::enable();
    task_switch_safe();
}
