use core::hash::Hash;

use alloc::borrow::Cow;
use spin::lock_api::Mutex;
use uuid::Uuid;
use x86_64::{VirtAddr, registers::control::Cr3Flags, structures::paging::PhysFrame};

use crate::{process::ProcessInfo, stack::SlabStack};

mod round_robin;
mod switching;

/// Low-level lock implementation for tasks.
pub mod lock;

pub use round_robin::change_current_process_info;
pub use round_robin::create_task;
pub use round_robin::get_current_process_info;
pub use round_robin::get_current_task;
pub use round_robin::get_current_task_id;
pub use round_robin::init;
pub use round_robin::set_current_process_info;
pub use round_robin::task_exit;
pub use round_robin::task_switch;
pub use round_robin::try_get_current_process_info;

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
    stack_pointer: VirtAddr,
    /// Active page table address of the task.
    cr3: (PhysFrame, Cr3Flags),
    /// Stack memory; freed when the task terminates.
    pub stack: Option<SlabStack>,
    /// Pointer to the task's owning process metadata.
    pub process_info: Option<ProcessInfo>,
    scheduler_data: Data,
}
