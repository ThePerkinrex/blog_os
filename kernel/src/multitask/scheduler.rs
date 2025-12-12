// pub use round_robin::create_task;
// pub use round_robin::get_current_task;
// pub use round_robin::try_get_current_task;
// pub use round_robin::init;
// pub use round_robin::task_exit;
// pub use round_robin::switch_fn;

use core::{
    num::{NonZeroUsize, Wrapping},
    sync::atomic::AtomicBool,
};

use alloc::{
    collections::{binary_heap::BinaryHeap, btree_map::BTreeMap, btree_set::BTreeSet},
    sync::{Arc, Weak},
    vec::Vec,
};
use spin::lock_api::RwLock;

use crate::multitask::{
    switching::SwitchData,
    task::{TaskControlBlock, free_task},
};

pub struct SchedulerData {
    dying: bool,
    sleeping: bool,
    priority: NonZeroUsize,
    vruntime: Wrapping<usize>,
    deadline: Wrapping<usize>,
}

struct Scheduler {
    current: RwLock<Arc<TaskControlBlock<SchedulerData>>>,
    last: RwLock<Weak<TaskControlBlock<SchedulerData>>>,
    ready: RwLock<BTreeSet<Arc<TaskControlBlock<SchedulerData>>>>,
    sleeping: RwLock<BTreeMap<uuid::Uuid, Arc<TaskControlBlock<SchedulerData>>>>,
    waking: RwLock<BTreeSet<Arc<TaskControlBlock<SchedulerData>>>>,
    needs_reschedule: AtomicBool,
}

fn get_scheduler<'a>() -> &'a Scheduler {
    todo!()
}

// This constant represents 2^(BITS - 1) for a usize.
const HALF_RANGE: Wrapping<usize> = Wrapping((usize::MAX / 2) + 1);

pub fn tick() {
    let scheduler = get_scheduler();
    let current = scheduler.current.read().clone();
    let mut ctx = current.context.lock();
    ctx.scheduler_data.vruntime += 1;
    let vruntime = ctx.scheduler_data.vruntime;
    let deadline = ctx.scheduler_data.deadline;
    drop(ctx);

    // 2. Implement the modular comparison: Is vruntime >= deadline?
    // We use wrapping_sub and check if the difference is <= half the maximum value.
    // If the difference (vruntime - deadline) is small, vruntime is "ahead".
    // If the difference is large, it means deadline is "ahead" (vruntime wrapped).
    let difference = vruntime - deadline;

    if difference < HALF_RANGE {
        // This condition is true if vruntime >= deadline (in a time-based sense)
        // If vruntime has passed or reached the deadline, a reschedule is needed.
        scheduler
            .needs_reschedule
            .store(true, core::sync::atomic::Ordering::Relaxed);
    }
}

const SLICE: Wrapping<usize> = Wrapping(10_000);

fn reschedule() {
    let scheduler = get_scheduler();
    let mut ready = scheduler.ready.write();

    // get the accumulated priority of all ready processes
    let mut total_priority: Option<NonZeroUsize> = None;
    let mut minimum_vruntime: Option<Wrapping<usize>> = None;
    for t in ready.iter() {
        let ctx = t.context.lock();
        total_priority = Some(total_priority.map_or(ctx.scheduler_data.priority, |p| {
            p.saturating_add(ctx.scheduler_data.priority.get())
        }));
        minimum_vruntime = Some(minimum_vruntime.map_or(
            ctx.scheduler_data.vruntime,
            |min_vruntime| {
                if min_vruntime - ctx.scheduler_data.vruntime < HALF_RANGE {
                    ctx.scheduler_data.vruntime
                } else {
                    min_vruntime
                }
            },
        ));
        drop(ctx);
    }

    let mut total_priority = total_priority.or(NonZeroUsize::new(1)).unwrap();
    let minimum_vruntime = minimum_vruntime.unwrap_or_default();

    let mut waking = scheduler.waking.write();
    while let Some(task) = waking.pop_last() {
        let mut ctx = task.context.lock();
        total_priority = total_priority.saturating_add(ctx.scheduler_data.priority.get());
        ctx.scheduler_data.vruntime = minimum_vruntime;
        drop(ctx);
        ready.insert(task);
    }
    drop(waking);

    // Compute the deadline for each ready process = vruntime + SLICE * (priority / total_priority)
    let mut soonest_deadline = None;
    for t in ready.iter() {
        let mut ctx = t.context.lock();

        ctx.scheduler_data.deadline = ctx.scheduler_data.vruntime
            + (SLICE * Wrapping(ctx.scheduler_data.priority.get()))
                / Wrapping(total_priority.get());
        soonest_deadline = Some(soonest_deadline.map_or_else(
            || (t.clone(), ctx.scheduler_data.deadline),
            |(a, b)| {
                if b - ctx.scheduler_data.deadline < HALF_RANGE {
                    (t.clone(), ctx.scheduler_data.deadline)
                } else {
                    (a, b)
                }
            },
        ));

        drop(ctx);
    }

    scheduler
        .needs_reschedule
        .store(false, core::sync::atomic::Ordering::Relaxed);

    if let Some(next) = soonest_deadline.map(|(a, _)| a) {
        ready.remove(&next);

        let mut current = scheduler.current.write();
        let last = core::mem::replace(&mut *current, next);
        drop(current);
        let last_weak = Arc::downgrade(&last);
        *scheduler.last.write() = last_weak;
        let ctx = last.context.lock();
        let dying = ctx.scheduler_data.dying;
        let sleeping = ctx.scheduler_data.sleeping;
        drop(ctx);

        if dying || sleeping {
            scheduler.sleeping.write().insert(last.id, last);
        } else {
            ready.insert(last);
        }
    } else {
        // TODO sleeping task
        panic!("No next task")
    }

    drop(ready);
}

pub fn switch_fn() -> SwitchData<SchedulerData> {
    let scheduler = get_scheduler();
    if scheduler
        .needs_reschedule
        .load(core::sync::atomic::Ordering::Acquire)
    {
        reschedule(); // Last is written always on reschedule
        let current = scheduler.last.read().upgrade().unwrap();
        let next = scheduler.current.read().clone();
        SwitchData { current, next }
    } else {
        let current = scheduler.current.read().clone();
        SwitchData {
            next: current.clone(),
            current,
        }
    }
}

pub fn after_switch() {
    // Should the last task die?
    let scheduler = get_scheduler();
    let mut last_lock = scheduler.last.write();
    if let Some(last) = last_lock.upgrade() {
        let ctx = last.context.lock();
        let dying = ctx.scheduler_data.dying;
        drop(ctx);

        if dying {
            *last_lock = Weak::new();
            drop(last_lock);
            drop(scheduler.sleeping.write().remove(&last.id));
            // the only ref remaining must be my ref
            if Arc::strong_count(&last) > 1 {
                panic!("More than one ref to dying task")
            }
            unsafe { free_task(last) };
        }
    }
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
