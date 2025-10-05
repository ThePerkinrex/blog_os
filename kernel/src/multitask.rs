use core::{
    arch::naked_asm,
    ptr,
};

use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use spin::{Lazy, Mutex};
use x86_64::{
    VirtAddr,
    instructions::interrupts,
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame},
};

use crate::{SetupInfo, hlt_loop, println};

#[derive(Debug)]
struct TaskControlBlock {
    stack_pointer: VirtAddr,
    cr3: (PhysFrame, Cr3Flags),
    next_task: Weak<Mutex<TaskControlBlock>>, // state?
}

static TASKS: Lazy<Mutex<Vec<Arc<Mutex<TaskControlBlock>>>>> = Lazy::new(|| {
    Mutex::new(alloc::vec![Arc::new_cyclic(|w| Mutex::new(
        TaskControlBlock {
            stack_pointer: VirtAddr::zero(),
            cr3: Cr3::read(),
            next_task: w.clone()
        }
    ))])
});

static CURRENT_TASK: Lazy<Mutex<Arc<Mutex<TaskControlBlock>>>> =
    Lazy::new(|| Mutex::new(TASKS.lock()[0].clone()));

/// # Safety
/// Interrupts must be disabled when calling this function
unsafe fn task_switch() {
    // 1) Grab the current Arc<Mutex<TaskControlBlock>>
    let mut current_arc_guard = CURRENT_TASK.lock();
    let current_arc = current_arc_guard.clone(); // Arc clone of current
    let mut current_tcb = current_arc.lock();

    // 2) Find the next task (upgrade Weak -> Arc)
    let next_arc = current_tcb
        .next_task
        .upgrade()
        .expect("next task has been dropped");
    let next_tcb = next_arc.lock();

    // 3) update global CURRENT_TASK to the next task (so future calls observe it)
    *current_arc_guard = next_arc.clone();
    drop(current_arc_guard); // release global mutex

    // 4) Switch page tables to the next task's CR3 before switching stack.
    //    This ensures the next task's stack addresses are valid after we load its rsp.
    let (next_frame, next_flags) = next_tcb.cr3;
    unsafe {
        Cr3::write(next_frame, next_flags);
    }

    // 5) Prepare pointers for asm:
    // Save pointer to current_tcb.stack_pointer so asm can write the current rsp into it.
    let cur_sp_ptr: *mut VirtAddr = &mut current_tcb.stack_pointer; // VirtAddr is transparent u64
    let next_sp_ptr: *const u64 = next_tcb.stack_pointer.as_ptr();

    // `current_tcb` and `next_tcb` are still borrowed here; we must drop them before asm
    // since asm can clobber memory/locks and we don't want the locks held across the asm call.
    drop(next_tcb);
    drop(current_tcb);

    unsafe {
        __switch_asm(cur_sp_ptr, next_sp_ptr);
    }
    // after the asm returns, execution continues in the context of the next task.
}

#[unsafe(naked)]
unsafe extern "C" fn __switch_asm(cur_sp_ptr: *mut VirtAddr, next_sp_ptr: *const u64) {
    naked_asm!(
        // save callee-saved registers (order must match how we build new stacks)
        "push r15",
        "push r14",
        "push r13",
        "push r12",
        "push rbx",
        "push rbp",
        // store old rsp into *rdi
        "mov [rdi], rsp",
        // switch to next stack (rsp <- rsi)
        "mov rsp, rsi",
        // restore callee-saved registers (popped in reverse order)
        "pop rbp",
        "pop rbx",
        "pop r12",
        "pop r13",
        "pop r14",
        "pop r15",
        // return into the next task's return address on its stack
        "ret",
    )
}

/// Perform a cooperative task switch safely.
///
/// This disables interrupts around the unsafe context switch,
/// ensuring the CPU state doesn't change mid-switch.
pub fn task_switch_safe() {
    interrupts::without_interrupts(|| {
        // Safety: we have disabled interrupts and are the only code modifying
        // the current CPU context. The task control blocks and stacks must
        // be valid and non-overlapping.
        unsafe {
            task_switch();
        }
    });
}

extern "C" fn task_exit() -> ! {
    println!("Not endind task ended");
    // Dealloc task
    // If a task returns, just halt for now
    hlt_loop()
}

pub fn create_task(entry: extern "C" fn(), setup: &mut SetupInfo) {
    let _ = TASKS.is_locked(); // Force lazy init
    let _ = CURRENT_TASK.is_locked(); // Force lazy init
    println!("Locks: {} {}", TASKS.is_locked(), CURRENT_TASK.is_locked());
    // === 1) Allocate a stack ===
    const STACK_PAGES: usize = 2;
    let page_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    // Pick a virtual address for the stack (for now, hardcode or use a kernel region allocator)
    let stack_top = VirtAddr::new_truncate(0xFFFF_FF00_0000_0000);
    let stack_bottom = stack_top - STACK_PAGES as u64 * 4096;

    let mapper = &mut setup.page_table;
    let frame_alloc = &mut setup.frame_allocator;

    for page in Page::range(
        Page::containing_address(stack_bottom),
        Page::containing_address(stack_top),
    ) {
        let frame = frame_alloc.allocate_frame().expect("no frames");
        unsafe {
            mapper
                .map_to(page, frame, page_flags, frame_alloc)
                .expect("map_to failed")
                .flush();
        }
    }

    println!("Created stack (top: {stack_top:p}; bottom: {stack_bottom:p})");

    // === 2) Prepare initial stack frame ===
    //
    // After task_switch, rsp will point to this frame and the function will `ret` into `entry`.
    let mut stack_ptr = stack_top.as_mut_ptr::<*const ()>(); // is aligned

    // reserve space
    let words = [
        ptr::null(), // rbp
        ptr::null(), // rbx
        ptr::null(), // r12
        ptr::null(), // r13
        ptr::null(), // r14
        ptr::null(), // r15
        entry as *const (),
        task_exit as *const (),
    ];

    for w in words.into_iter().rev() {
        // Last is first to enter stack
        stack_ptr = unsafe { stack_ptr.sub(1) };
        println!("{stack_ptr:p}: {w:p}");
        unsafe {
            core::ptr::write_volatile(stack_ptr, w);
        }
    }
    println!("Allocated stack (sp {stack_ptr:p})");

    let tcb = Arc::new_cyclic(|weak_self| {
        Mutex::new(TaskControlBlock {
            stack_pointer: VirtAddr::from_ptr(stack_ptr),
            cr3: Cr3::read(),
            next_task: Weak::clone(weak_self),
        })
    });
    println!("Created tcb pointing to itself");

    // === 3) Add to task list after current ===
    {
        let mut tasks = TASKS.lock();
        println!("Locked tasks");
        let current = CURRENT_TASK.lock();
        println!("Locked current task");
        tasks.push(tcb.clone());

        println!("Pushed tcb");

        // Fix linked list
        let mut cur_tcb = current.lock();
        let cur_next = cur_tcb.next_task.clone();
        cur_tcb.next_task = Arc::downgrade(&tcb);
        let mut new_tcb = tcb.lock();
        new_tcb.next_task = cur_next;
    }

    println!("Finished");
}
