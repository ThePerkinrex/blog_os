use alloc::{
    borrow::Cow,
    sync::{Arc, Weak},
};
use core::{hash::Hash, ptr};
use spin::lock_api::Mutex;
use uuid::Uuid;
use x86_64::{
    VirtAddr,
    registers::control::{Cr3, Cr3Flags},
    structures::paging::PhysFrame,
};

use crate::{process::ProcessInfo, rand::uuid_v4, setup::KERNEL_INFO, stack::SlabStack};

/// Optional task ID type.
pub type TaskId = Option<Uuid>;

/// A Task Control Block (TCB) stores everything needed to manage and switch
/// between tasks. This includes the task's ID, name, and its CPU context.
pub struct TaskControlBlock<Data> {
    /// Unique identifier for the task.
    pub id: Uuid,
    /// Human-readable name for debugging.
    pub name: Cow<'static, str>,
    /// Saved CPU state (stack pointer, CR3, etc.).
    pub context: Mutex<Context<Data>>,
}

// === Ordering + Hashing for BTreeSet ===
impl<Data> Ord for TaskControlBlock<Data> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}
impl<Data> PartialOrd for TaskControlBlock<Data> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<Data> PartialEq for TaskControlBlock<Data> {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}
impl<Data> Eq for TaskControlBlock<Data> {}
impl<Data> Hash for TaskControlBlock<Data> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

/// CPU execution context of a task.
/// Contains all information needed to resume execution.
pub struct Context<Data> {
    /// Saved stack pointer (rsp).
    pub(super) stack_pointer: VirtAddr,
    /// Active page table address of the task.
    pub(super) cr3: (PhysFrame, Cr3Flags),
    /// Stack memory; freed when the task terminates.
    pub stack: Option<SlabStack>,
    /// Pointer to the task's owning process metadata.
    pub process_info: Option<ProcessInfo>,
    pub(super) scheduler_data: Data,
}

/// Creates a new task whose `next_task` points back to itself.
pub(super) fn create_cyclic_task<S: Into<Cow<'static, str>>, Data>(
    entry: extern "C" fn(),
    name: S,
    task_exit: extern "C" fn() -> !,
    load: impl FnOnce(),
    data: impl FnOnce(&Weak<TaskControlBlock<Data>>) -> Data,
) -> Arc<TaskControlBlock<Data>> {
    load();

    let stack = KERNEL_INFO.get().unwrap().create_stack().expect("A stack");

    // Prepare the initial stack frame so that switching into it causes `entry` to run.
    let mut stack_ptr = stack.top().as_mut_ptr::<*const ()>();

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
        stack_ptr = unsafe { stack_ptr.sub(1) };
        unsafe { core::ptr::write_volatile(stack_ptr, w) };
    }

    let name = name.into();

    Arc::new_cyclic(|weak_self| TaskControlBlock {
        name,
        id: uuid_v4(),
        context: Mutex::new(Context {
            stack_pointer: VirtAddr::from_ptr(stack_ptr),
            cr3: Cr3::read(),
            stack: Some(stack),
            process_info: None,
            scheduler_data: data(weak_self),
        }),
    })
}
