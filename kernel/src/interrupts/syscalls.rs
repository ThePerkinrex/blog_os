use log::{debug, warn};

use crate::{multitask::task_switch_safe, unwind};

type SyscallHandler = fn(u64, u64, u64, u64, u64, u64) -> u64;

const SYSCALL_HANDLERS: &[SyscallHandler] = &[nop, exit];

fn nop(arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64, arg6: u64) -> u64 {
    debug!("NOP SYSCALL ({arg1}, {arg2}, {arg3}, {arg4}, {arg5}, {arg6})");
    0
}

fn exit(code: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    debug!("EXIT SYSCALL ({code})");
    !code
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
    unwind::backtrace();
    if code < SYSCALL_HANDLERS.len() as u64 {
        SYSCALL_HANDLERS[code as usize](arg1, arg2, arg3, arg4, arg5, arg6)
    } else {
        warn!("Unknown syscall {code} ({arg1}, {arg2}, {arg3}, {arg4}, {arg5}, {arg6})");
        u64::MAX
    }
}

pub extern "C" fn syscall_tail() {
    x86_64::instructions::interrupts::enable();
    debug!("SYSCALL TAIL");
    unwind::backtrace();
    task_switch_safe();
    unwind::backtrace();
}
