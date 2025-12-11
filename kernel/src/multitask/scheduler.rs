// pub use round_robin::create_task;
// pub use round_robin::get_current_task;
// pub use round_robin::try_get_current_task;
// pub use round_robin::init;
// pub use round_robin::task_exit;
// pub use round_robin::switch_fn;

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
}

struct Scheduler {
    current: RwLock<Arc<TaskControlBlock<SchedulerData>>>,
    last: RwLock<Weak<TaskControlBlock<SchedulerData>>>,
    ready: RwLock<BinaryHeap<Arc<TaskControlBlock<SchedulerData>>>>,
    sleeping: RwLock<BTreeMap<uuid::Uuid, Arc<TaskControlBlock<SchedulerData>>>>,
}

fn get_scheduler<'a>() -> &'a Scheduler {
    todo!()
}

pub fn wake(id: &uuid::Uuid) {
	let scheduler = get_scheduler();
    let Some(task) = scheduler.sleeping.write().remove(id) else {
        return;
    };

	let ready = scheduler.ready.read();
	let mut start = 0;
	let mut end = ready.len();
	while start < end {
		let middle = (start + end) / 2;

	}
	
}
