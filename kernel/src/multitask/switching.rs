use core::arch::naked_asm;

use alloc::sync::Arc;
use log::{debug, info};
use x86_64::{VirtAddr, instructions::interrupts, registers::control::Cr3};

use crate::{multitask::TaskControlBlock, setup::KERNEL_INFO};

/// Naked assembly function that performs the actual register + stack switching.
#[unsafe(naked)]
unsafe extern "C" fn __switch_asm(cur_sp_ptr: *mut VirtAddr, next_sp_ptr: *const u64) {
    naked_asm!(
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push rbx",
        "push rbp",
        // Save old RSP
        "mov [rdi], rsp",
        // Load new RSP
        "mov rsp, rsi",
        // Restore registers
        "pop rbp",
        "pop rbx",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",
        "ret",
    )
}

pub struct SwitchData<Data> {
    pub current: Arc<TaskControlBlock<Data>>,
    pub next: Arc<TaskControlBlock<Data>>,
}

/// # Safety
/// Performs a raw context switch between tasks.
/// Interrupts MUST be disabled.
unsafe fn task_switch<Data>(switch_fn: fn() -> SwitchData<Data>) {
    let SwitchData { current, next } = switch_fn();

    if Arc::ptr_eq(&current, &next) {
        return;
    }

    debug!("Locking current tcb");
    let mut current_tcb = current.context.lock();
    debug!("Locked current tcb");

    debug!("Locking next tcb");
    let next_tcb = next.context.lock();
    debug!("Locked next tcb");

    // Switch page tables (CR3) if needed
    let (next_frame, next_flags) = next_tcb.cr3;
    let cur_cr3 = Cr3::read();
    if cur_cr3.0 != next_frame || cur_cr3.1 != next_flags {
        info!(
            "Switching CR3 from {cur_cr3:?} to {:?}",
            (next_frame, next_flags)
        );
        unsafe {
            Cr3::write(next_frame, next_flags);
        }

        // Update kernel allocator page table
        let mut lock = KERNEL_INFO.get().unwrap().alloc_kinf.lock();
        let mem = &mut *lock;
        mem.page_table
            .set_current_page_table(&mut mem.frame_allocator);
        drop(lock)
    }

    // Save old cr3
    current_tcb.cr3 = cur_cr3;

    // Prepare stack pointer pointers
    let cur_sp_ptr: *mut VirtAddr = &mut current_tcb.stack_pointer;
    let next_sp_ptr: *const u64 = next_tcb.stack_pointer.as_ptr();

    drop(next_tcb);
    drop(current_tcb);

    unsafe {
        __switch_asm(cur_sp_ptr, next_sp_ptr);
    }

    debug!("Current sp: {cur_sp_ptr:p}");
    debug!("Next sp: {next_sp_ptr:p}");
}

/// Safe wrapper around `task_switch`, ensuring interrupts are disabled.
pub fn task_switch_safe<Data>(switch_fn: fn() -> SwitchData<Data>) {
    interrupts::without_interrupts(|| unsafe { task_switch(switch_fn) });
}
