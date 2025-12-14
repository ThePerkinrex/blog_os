use crate::multitask::task::TaskId;
use crate::process::ProcessInfo;

mod switching;
pub mod task;

mod round_robin;
mod scheduler;

/// Low-level lock implementation for tasks.
pub mod lock;

pub use round_robin::create_task;
pub use round_robin::get_current_task;
pub use round_robin::init;
use round_robin::switch_fn;
pub use round_robin::task_exit;
pub use round_robin::try_get_current_task;

const fn do_nothing() {}

pub fn task_switch() -> bool {
    switching::task_switch_safe(switch_fn, do_nothing)
}

/// Returns the ID of the current task.
pub fn get_current_task_id() -> TaskId {
    try_get_current_task().map(|x| x.id)
}

/// Sets process metadata for the current task.
pub fn set_current_process_info(process_info: ProcessInfo) {
    let arc = get_current_task();
    arc.context.lock().process_info = Some(process_info);
}

/// Mutably modifies the current task's process info.
#[allow(clippy::significant_drop_tightening)]
pub fn change_current_process_info<U>(f: impl Fn(&mut Option<ProcessInfo>) -> U) -> U {
    let lock = get_current_task();
    let p = &mut lock.context.lock().process_info;
    f(p)
}

/// Returns a copy of the current task's process info.
pub fn get_current_process_info() -> Option<ProcessInfo> {
    get_current_task().context.lock().process_info.clone()
}

/// Attempts to get process info; returns None if scheduler not initialized.
pub fn try_get_current_process_info() -> Option<ProcessInfo> {
    try_get_current_task().and_then(|x| x.context.try_lock()?.process_info.clone())
}
