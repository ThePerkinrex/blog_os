use core::ops::{Index, IndexMut};

use alloc::sync::Arc;
use blog_os_syscalls::SyscallNumber;
use log::{debug, info, warn};
use spin::Lazy;

use crate::{
    multitask::{
        change_current_process_info, task_exit, task_switch, try_get_current_process_info,
    },
    process::ProcessStatus,
};

mod brk;
mod close;
mod exit;
mod flush;
mod init_driver;
mod next_direntry;
mod nop;
mod open;
mod read;
mod stat;
mod write;
mod yield_syscall;

type SyscallHandler = fn(u64, u64, u64, u64, u64, u64) -> u64;

struct SyscallHandlers([SyscallHandler; SyscallNumber::MAX_PRIMITIVE + 1]);

impl Index<SyscallNumber> for SyscallHandlers {
    type Output = SyscallHandler;

    fn index(&self, index: SyscallNumber) -> &Self::Output {
        &self.0[usize::from(index)]
    }
}

impl IndexMut<SyscallNumber> for SyscallHandlers {
    fn index_mut(&mut self, index: SyscallNumber) -> &mut Self::Output {
        &mut self.0[usize::from(index)]
    }
}

static SYSCALL_HANDLERS: Lazy<SyscallHandlers> = Lazy::new(|| {
    let mut nums = SyscallHandlers([nop::nop; SyscallNumber::MAX_PRIMITIVE + 1]);

    nums[SyscallNumber::NOP] = nop::nop;
    nums[SyscallNumber::EXIT] = exit::exit;
    nums[SyscallNumber::WRITE] = write::write;
    nums[SyscallNumber::BRK] = brk::brk;
    nums[SyscallNumber::YIELD] = yield_syscall::yield_syscall;
    nums[SyscallNumber::READ] = read::read;
    nums[SyscallNumber::OPEN] = open::open;
    nums[SyscallNumber::CLOSE] = close::close;
    nums[SyscallNumber::FLUSH] = flush::flush;
    nums[SyscallNumber::STAT] = stat::stat;
    nums[SyscallNumber::NEXT_DIRENTRY] = next_direntry::next_direntry;
    nums[SyscallNumber::INIT_DRIVER] = init_driver::init_driver;

    nums
});

pub fn syscall_handle(
    code: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
) -> u64 {
    debug!("Syscall: {code}");
    // unwind::backtrace();
    // debug!("Unwound stack");
    match SyscallNumber::try_from(code) {
        Ok(code) => {
            debug!("Syscall: {code:?}");
            SYSCALL_HANDLERS[code](arg1, arg2, arg3, arg4, arg5, arg6)
        }
        Err(e) => {
            warn!(
                "Unknown syscall {code} ({arg1}, {arg2}, {arg3}, {arg4}, {arg5}, {arg6}) [error: {e:?}]"
            );
            u64::MAX
        }
    }
}

pub extern "C" fn syscall_tail() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // debug!("SYSCALL TAIL");

        if let Some(current_pinf) = try_get_current_process_info() {
            // Process info must be there if a syscall was made.

            if let ProcessStatus::Ending(code) = current_pinf.status() {
                info!("Process ending with code: {code}");
                change_current_process_info(|p| p.take()); // This process is no longer associated with the task
                let program = current_pinf.program().clone();
                drop(current_pinf);

                debug!("Strong count: {}", Arc::strong_count(&program));
                debug!("Weak count: {}", Arc::weak_count(&program));

                let program = Arc::into_inner(program).expect("No more than one ref");
                drop(program);

                task_exit();
            }

            drop(current_pinf);
        }
        task_switch();
    });
    x86_64::instructions::interrupts::enable();
}
