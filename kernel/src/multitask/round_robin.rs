use core::sync::atomic::AtomicBool;

use alloc::{
    borrow::Cow,
    collections::BTreeSet,
    sync::{Arc, Weak},
};
use log::{debug, info, warn};
use spin::{
    Lazy, Once,
    lock_api::{Mutex, RwLock},
};
use x86_64::{VirtAddr, registers::control::Cr3};

use crate::{
    multitask::{
        switching::SwitchData,
        task::{self, Context, TaskControlBlock, free_task},
        task_switch,
    },
    rand::uuid_v4,
};

pub struct RoundRobinData {
    /// Weak pointer to the next task in the run queue.
    next_task: Weak<TaskControlBlock<Self>>,
    /// Task scheduled for deallocation.
    dealloc: Option<Arc<TaskControlBlock<Self>>>,
    time: usize,
}

/// Tracks whether the scheduler has been initialized.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Global set of runnable tasks.
static TASKS: Lazy<Mutex<BTreeSet<Arc<TaskControlBlock<RoundRobinData>>>>> = Lazy::new(|| {
    INITIALIZED.store(true, core::sync::atomic::Ordering::Release);

    // Create the initial 'init' task.
    #[allow(clippy::mutable_key_type)] // The ordering is not mutable
    let mut set = BTreeSet::new();
    set.insert(Arc::new_cyclic(|w| TaskControlBlock {
        id: uuid_v4(),
        name: "init".into(),
        context: Mutex::new(Context {
            stack_pointer: VirtAddr::zero(),
            cr3: Cr3::read(),
            stack: None,
            process_info: None,
            scheduler_data: RoundRobinData {
                next_task: w.clone(),
                dealloc: None,
                time: 0,
            },
        }),
    }));

    Mutex::new(set)
});

/// The currently running task.
static CURRENT_TASK: Lazy<RwLock<Arc<TaskControlBlock<RoundRobinData>>>> =
    Lazy::new(|| RwLock::new(TASKS.lock().first().unwrap().clone()));

const TIME_LIMIT: usize = 100;

pub fn switch_fn() -> SwitchData<RoundRobinData> {
    let mut current_arc_guard = CURRENT_TASK.write();
    let current_arc = current_arc_guard.clone();
    // debug!("Locking current tcb");
    let mut current_tcb = current_arc.context.lock();

    if current_tcb.scheduler_data.time < TIME_LIMIT {
        current_tcb.scheduler_data.time += 1;
        return SwitchData {
            current: current_arc.clone(),
            next: current_arc.clone(),
        };
    }
    current_tcb.scheduler_data.time = 0;

    // debug!("Locked current tcb");

    // Upgrade next_task Weak → Arc
    let next_arc: Arc<_> = current_tcb
        .scheduler_data
        .next_task
        .upgrade()
        .expect("next task has been dropped");

    if Arc::ptr_eq(&current_arc, &next_arc) {
        info!("Called task switch on a cyclic task, returning");
        return SwitchData {
            current: current_arc.clone(),
            next: current_arc.clone(),
        };
    }

    info!(
        "Switching from {} ({}) to {} ({})",
        current_arc.name, current_arc.id, next_arc.name, next_arc.id
    );

    // Update global current task pointer
    *current_arc_guard = next_arc.clone();
    drop(current_arc_guard);
    drop(current_tcb);

    SwitchData {
        current: current_arc,
        next: next_arc,
    }
}

fn create_cyclic_task<S: Into<Cow<'static, str>>>(
    entry: extern "C" fn(),
    name: S,
) -> Arc<TaskControlBlock<RoundRobinData>> {
    task::create_cyclic_task(
        entry,
        name,
        task_exit,
        || {
            let _ = TASKS.is_locked();
            let _ = CURRENT_TASK.is_locked();
            debug!("Creating task. loaded current_task");
        },
        |weak_self| RoundRobinData {
            next_task: Weak::clone(weak_self),
            dealloc: None,
            time: 0,
        },
    )
}

/// Public task creation function; inserts the task after the current one.
pub fn create_task(entry: extern "C" fn(), name: &'static str) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let tcb = create_cyclic_task(entry, name);

        {
            let mut tasks = TASKS.lock();
            let current = CURRENT_TASK.read().clone();
            tasks.insert(tcb.clone());
            drop(tasks);

            // Fix linked list
            let mut cur_tcb = current.context.lock();
            let old_next = cur_tcb.scheduler_data.next_task.clone();
            cur_tcb.scheduler_data.next_task = Arc::downgrade(&tcb);
            drop(cur_tcb);
            let mut new_tcb = tcb.context.lock();
            new_tcb.scheduler_data.next_task = old_next;
        }

        info!("Task creation finished");
    });
}

/// Special task for performing cleanup of dead tasks.
static TASK_DEALLOC: Once<Arc<TaskControlBlock<RoundRobinData>>> = Once::new();

/// Initializes the deallocation task.
pub fn init() {
    info!("Initializing mustitasking");
    x86_64::instructions::interrupts::without_interrupts(|| {
        let dealloc = create_cyclic_task(task_dealloc, "dealloc");
        TASKS.lock().insert(dealloc.clone());
        TASK_DEALLOC.call_once(|| dealloc);
        info!("Initialized mustitasking");
    });
}

/// Marks the current task for deallocation and switches to the
/// deallocation task.
pub extern "C" fn task_exit() -> ! {
    x86_64::instructions::interrupts::without_interrupts(|| {
        info!("Ending task");
        let dealloc = TASK_DEALLOC.get().expect("Initialized dealloc");
        let current = CURRENT_TASK.read();

        let old_next = {
            let mut cur = current.context.lock();
            if cur
                .scheduler_data
                .next_task
                .ptr_eq(&Arc::downgrade(&current))
            {
                panic!("Ending the only task. This task is cyclic")
            }
            let old = cur.scheduler_data.next_task.clone();
            cur.scheduler_data.next_task = Arc::downgrade(dealloc);
            old
        };

        {
            let mut del = dealloc.context.lock();
            del.scheduler_data.next_task = old_next;
            del.scheduler_data.dealloc = Some(current.clone());
        }

        drop(current);

        info!("Switching to dealloc");
        task_switch();
        unreachable!();
    })
}

/// Task dedicated to freeing task resources.
extern "C" fn task_dealloc() {
    loop {
        if let Some(dealloc_ptr) = TASK_DEALLOC.get() {
            let mut dealloc_ptr_lock = dealloc_ptr.context.lock();
            if let Some(dealloc_task_ptr) = dealloc_ptr_lock.scheduler_data.dealloc.take() {
                info!(
                    "Cleaning up {} ({})",
                    dealloc_task_ptr.name, dealloc_task_ptr.id
                );

                let mut tasks = TASKS.lock();
                tasks.remove(&dealloc_task_ptr);
                info!("Removed task from list");

                // Redirect tasks that pointed to the now-dead task
                for t in tasks.iter() {
                    if t != dealloc_ptr {
                        let mut lock = t.context.lock();
                        if lock
                            .scheduler_data
                            .next_task
                            .ptr_eq(&Arc::downgrade(&dealloc_task_ptr))
                        {
                            debug!("{} ({}) pointed to this task, rerouting", t.name, t.id);
                            lock.scheduler_data.next_task =
                                dealloc_ptr_lock.scheduler_data.next_task.clone();
                        }
                    }
                }

                drop(tasks);
                drop(dealloc_ptr_lock);

                unsafe { free_task(dealloc_task_ptr) };
            } else {
                drop(dealloc_ptr_lock);
                info!("Nothing to clean up");
            }
        } else {
            warn!("Dealloc not initialized");
        }

        task_switch();
    }
}

/// Returns the currently active task.
pub fn get_current_task() -> Arc<TaskControlBlock<RoundRobinData>> {
    CURRENT_TASK.read().clone()
}

pub fn try_get_current_task() -> Option<Arc<TaskControlBlock<RoundRobinData>>> {
    if INITIALIZED.load(core::sync::atomic::Ordering::Acquire) {
        CURRENT_TASK.try_read().map(|x| x.clone())
    } else {
        None
    }
}
