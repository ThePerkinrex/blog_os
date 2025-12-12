// pub use round_robin::create_task;
// pub use round_robin::get_current_task;
// pub use round_robin::try_get_current_task;
// pub use round_robin::init;
// pub use round_robin::task_exit;
// pub use round_robin::switch_fn;

use core::sync::atomic::AtomicBool;

use alloc::{
    collections::{binary_heap::BinaryHeap, btree_map::BTreeMap},
    sync::{Arc, Weak},
    vec::Vec,
};
use spin::lock_api::RwLock;

use crate::multitask::task::TaskControlBlock;

pub struct SchedulerData {
    dying: bool,
    priority: usize,
    vruntime: usize,
    deadline: usize,
}

struct Scheduler {
    current: RwLock<Arc<TaskControlBlock<SchedulerData>>>,
    last: RwLock<Weak<TaskControlBlock<SchedulerData>>>,
    ready: RwLock<BinaryHeap<Arc<TaskControlBlock<SchedulerData>>>>,
    sleeping: RwLock<BTreeMap<uuid::Uuid, Arc<TaskControlBlock<SchedulerData>>>>,
    needs_reschedule: AtomicBool,
}

fn get_scheduler<'a>() -> &'a Scheduler {
    todo!()
}

pub fn tick() {
    let scheduler = get_scheduler();
    let current = scheduler.current.read().clone();
    let mut ctx = current.context.lock();
    ctx.scheduler_data.vruntime = ctx.scheduler_data.vruntime.wrapping_add(1);
    let vruntime = ctx.scheduler_data.vruntime;
    let deadline = ctx.scheduler_data.deadline;
    drop(ctx);

    // 2. Implement the modular comparison: Is vruntime >= deadline?
    // We use wrapping_sub and check if the difference is <= half the maximum value.
    // If the difference (vruntime - deadline) is small, vruntime is "ahead".
    // If the difference is large, it means deadline is "ahead" (vruntime wrapped).
    let difference = vruntime.wrapping_sub(deadline);

    // This constant represents 2^(BITS - 1) for a usize.
    const HALF_RANGE: usize = (usize::MAX / 2) + 1;

    if difference < HALF_RANGE {
        // This condition is true if vruntime >= deadline (in a time-based sense)
        // If vruntime has passed or reached the deadline, a reschedule is needed.
        scheduler
            .needs_reschedule
            .store(true, core::sync::atomic::Ordering::Relaxed);
    }
}

const SLICE: usize = 10_000;

fn reschedule() {
    let scheduler = get_scheduler();

    // get the accumulated priority of all ready processes

    // Compute the deadline for each ready process = vruntime + SLICE * (priority / total_priority)

    scheduler
        .needs_reschedule
        .store(false, core::sync::atomic::Ordering::Relaxed);
}

// pub fn wake(id: &uuid::Uuid) {
// 	let scheduler = get_scheduler();
//     let Some(task) = scheduler.sleeping.write().remove(id) else {
//         return;
//     };

// 	let ready = scheduler.ready.read();
// 	let mut start = 0;
// 	let mut end = ready.len();
// 	while start < end {
// 		let middle = (start + end) / 2;

// 	}

// }
