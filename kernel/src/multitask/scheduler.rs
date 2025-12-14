use core::{
    num::{NonZeroUsize, Wrapping},
    sync::atomic::AtomicBool,
};

use alloc::{
    borrow::Cow,
    collections::{btree_map::BTreeMap, btree_set::BTreeSet, vec_deque::VecDeque},
    sync::{Arc, Weak},
};
use log::{debug, info};
use spin::{
    Once,
    lock_api::{Mutex, RwLock},
};
use x86_64::{VirtAddr, registers::control::Cr3};

use crate::{
    multitask::{
        switching::SwitchData,
        task::{Context, TaskControlBlock, create_cyclic_task, free_task}, task_switch,
    },
    rand::uuid_v4,
};

#[derive(Debug)]
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
    waking: RwLock<VecDeque<Arc<TaskControlBlock<SchedulerData>>>>,
    needs_reschedule: AtomicBool,
}

static SCHED: Once<Scheduler> = Once::new();

fn get_scheduler<'a>() -> &'a Scheduler {
    SCHED.call_once(Scheduler::new)
}

const SLICE: Wrapping<usize> = Wrapping(10_000);
// This constant represents 2^(BITS - 1) for a usize.
const HALF_RANGE: Wrapping<usize> = Wrapping((usize::MAX / 2) + 1);

impl Scheduler {
    fn new() -> Self {
        info!("Initializing scheduler");
        let init = Arc::new_cyclic(|_| TaskControlBlock {
            id: uuid_v4(),
            name: "init".into(),
            context: Mutex::new(Context {
                stack_pointer: VirtAddr::zero(),
                cr3: Cr3::read(),
                stack: None,
                process_info: None,
                scheduler_data: SchedulerData {
                    dying: false,
                    sleeping: false,
                    priority: NonZeroUsize::new(1).unwrap(),
                    vruntime: core::num::Wrapping(0),
                    deadline: core::num::Wrapping(0),
                },
            }),
        });
        let init_weak = Arc::downgrade(&init);
        info!("Initialized scheduler");
        Self {
            current: RwLock::new(init),
            last: RwLock::new(init_weak),
            ready: Default::default(),
            sleeping: Default::default(),
            waking: Default::default(),
            needs_reschedule: Default::default(),
        }
    }

    fn tick(&self) {
        let current = self.current.read().clone();
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
            self.needs_reschedule
                .store(true, core::sync::atomic::Ordering::Relaxed);
        }
    }

    fn reschedule(&self) {
        info!("Reschedule");
        let mut ready = self.ready.write();

        let current = self.current.read().clone();
        let current_ctx = current.context.lock();

        // get the accumulated priority of all ready processes
        let mut total_priority: NonZeroUsize = current_ctx.scheduler_data.priority;
        let mut minimum_vruntime: Wrapping<usize> = current_ctx.scheduler_data.vruntime;
        drop(current_ctx);
        for t in ready.iter() {
            let ctx = t.context.lock();
            total_priority = total_priority.saturating_add(ctx.scheduler_data.priority.get());
            minimum_vruntime = if ctx.scheduler_data.vruntime - minimum_vruntime < HALF_RANGE {
                minimum_vruntime
            } else {
                ctx.scheduler_data.vruntime
            };
            drop(ctx);
        }

        let mut waking = self.waking.write();
        while let Some(task) = waking.pop_back() {
            let mut ctx = task.context.lock();
            total_priority = total_priority.saturating_add(ctx.scheduler_data.priority.get());
            ctx.scheduler_data.vruntime = minimum_vruntime;
            drop(ctx);
            ready.insert(task);
        }
        drop(waking);

        // Compute the deadline for each ready process = vruntime + SLICE * (priority / total_priority)
        let mut current_ctx = current.context.lock();
        current_ctx.scheduler_data.deadline = current_ctx.scheduler_data.vruntime
            + (SLICE * Wrapping(current_ctx.scheduler_data.priority.get()))
                / Wrapping(total_priority.get());
        let current_deadline = current_ctx.scheduler_data.deadline;
        let not_ready = current_ctx.scheduler_data.dying || current_ctx.scheduler_data.sleeping;
        drop(current_ctx);
        let mut soonest_deadline = if not_ready {
            None
        } else {
            Some((current, current_deadline))
        };
        debug!("Current task: not_ready: {not_ready} // {soonest_deadline:?}");
        for t in ready.iter() {
            let mut ctx = t.context.lock();

            ctx.scheduler_data.deadline = ctx.scheduler_data.vruntime
                + (SLICE * Wrapping(ctx.scheduler_data.priority.get()))
                    / Wrapping(total_priority.get());
            soonest_deadline = Some(soonest_deadline.map_or_else(
                || (t.clone(), ctx.scheduler_data.deadline),
                |(a, b)| {
                    if ctx.scheduler_data.deadline - b < HALF_RANGE {
                        (t.clone(), ctx.scheduler_data.deadline)
                    } else {
                        (a, b)
                    }
                },
            ));

            drop(ctx);
        }
        debug!("Soonest deadline is some? {:?}", soonest_deadline.is_some());

        self.needs_reschedule
            .store(false, core::sync::atomic::Ordering::Release);

        if let Some(next) = soonest_deadline.map(|(a, _)| a) {
            ready.remove(&next);

            let mut current = self.current.write();
            let last = core::mem::replace(&mut *current, next);
            let no_switch = Arc::ptr_eq(&last, &current);

            debug!("Replacing {} ({}) with {} ({})", last.name, last.id, current.name, current.id);
            drop(current);
            let last_weak = Arc::downgrade(&last);
            *self.last.write() = last_weak;
            let ctx = last.context.lock();
            let dying = ctx.scheduler_data.dying;
            let sleeping = ctx.scheduler_data.sleeping;
            drop(ctx);

            if dying || sleeping {
                self.sleeping.write().insert(last.id, last);
            } else if !no_switch {
                ready.insert(last);
            }
        } else {
            // TODO sleeping task
            todo!("No next task")
        }

        drop(ready);
    }

    fn create_task<S: Into<Cow<'static, str>>>(&self, entry: extern "C" fn(), name: S) {
        let task = create_cyclic_task(
            entry,
            name,
            task_exit,
            || {},
            |_| SchedulerData {
                dying: false,
                sleeping: false,
                priority: NonZeroUsize::new(1).unwrap(),
                vruntime: core::num::Wrapping(0),
                deadline: core::num::Wrapping(0),
            },
        );

        self.ready.write().insert(task);
    }
}

pub fn switch_fn() -> SwitchData<SchedulerData> {
    let scheduler = get_scheduler();
    let not_ready = {
        let current = scheduler.current.read().clone();
        let current_ctx = current.context.lock();
        current_ctx.scheduler_data.dying || current_ctx.scheduler_data.sleeping
    };
    if !not_ready {
        scheduler.tick();
    }
    if not_ready || scheduler
        .needs_reschedule
        .load(core::sync::atomic::Ordering::Acquire)
    {
        scheduler.reschedule(); // Last is written always on reschedule
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
                panic!("More than one ref to dying task. This shouldnt have happened")
            }
            unsafe { free_task(last) };
        }
    }
}

pub fn try_get_current_task() -> Option<Arc<TaskControlBlock<SchedulerData>>> {
    Some(SCHED.get()?.current.try_read()?.clone())
}

pub fn locking_get_current_task() -> Option<Arc<TaskControlBlock<SchedulerData>>> {
    Some(SCHED.get()?.current.read().clone())
}

pub fn init() {
    get_scheduler(); // Force initialization
}

pub extern "C" fn task_exit() -> ! {
    info!("Ending task");

    let current = get_scheduler().current.read().clone();
    current.context.lock().scheduler_data.dying = true;

    info!("Switching out of dying task");
    task_switch();
    unreachable!();
}

pub fn create_task<S: Into<Cow<'static, str>>>(entry: extern "C" fn(), name: S) {
    get_scheduler().create_task(entry, name);

    info!("Task creation finished");
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
