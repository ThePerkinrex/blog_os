use core::{arch::naked_asm, hash::Hash, ptr, sync::atomic::AtomicBool};

use alloc::{
    borrow::Cow,
    collections::BTreeSet,
    sync::{Arc, Weak},
};
use spin::{Lazy, Once, lock_api::Mutex};
use uuid::Uuid;
use x86_64::{
    VirtAddr,
    instructions::interrupts,
    registers::control::{Cr3, Cr3Flags},
    structures::paging::PhysFrame,
};

use crate::{KERNEL_INFO, println, process::ProcessInfo, rand::uuid_v4, stack::SlabStack};

pub mod lock;

#[derive(Debug)]
pub struct TaskControlBlock {
    pub id: Uuid,
    pub name: Cow<'static, str>,
    pub context: Mutex<Context>,
}

impl Ord for TaskControlBlock {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for TaskControlBlock {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for TaskControlBlock {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl Eq for TaskControlBlock {}

impl Hash for TaskControlBlock {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

#[derive(Debug)]
pub struct Context {
    stack_pointer: VirtAddr,
    cr3: (PhysFrame, Cr3Flags),
    next_task: Weak<TaskControlBlock>,
    pub stack: Option<SlabStack>,
    dealloc: Option<Arc<TaskControlBlock>>,
    pub process_info: Option<ProcessInfo>,
}

static INITIALIZED: AtomicBool = AtomicBool::new(false);

static TASKS: Lazy<Mutex<BTreeSet<Arc<TaskControlBlock>>>> = Lazy::new(|| {
    INITIALIZED.store(true, core::sync::atomic::Ordering::Release);
    #[allow(clippy::mutable_key_type)]
    let mut set = BTreeSet::new();
    set.insert(Arc::new_cyclic(|w| TaskControlBlock {
        id: uuid_v4(),
        name: "init".into(),
        context: Mutex::new(Context {
            stack_pointer: VirtAddr::zero(),
            cr3: Cr3::read(),
            next_task: w.clone(),
            stack: None,
            dealloc: None,
            process_info: None,
        }),
    }));
    Mutex::new(set)
});

// static ENDING_TASKS: Lazy<Mutex<BTreeSet<Arc<TaskControlBlock>>>> = Lazy::new(|| Mutex::new(BTreeSet::new()));

static CURRENT_TASK: Lazy<Mutex<Arc<TaskControlBlock>>> =
    Lazy::new(|| Mutex::new(TASKS.lock().first().unwrap().clone()));

pub fn get_current_task() -> Arc<TaskControlBlock> {
    CURRENT_TASK.lock().clone()
}

/// # Safety
/// Interrupts must be disabled when calling this function
unsafe fn task_switch() {
    // 1) Grab the current Arc<Mutex<TaskControlBlock>>
    let mut current_arc_guard = CURRENT_TASK.lock();
    let current_arc = current_arc_guard.clone(); // Arc clone of current
    let mut current_tcb = current_arc.context.lock();

    // 2) Find the next task (upgrade Weak -> Arc)
    let next_arc: Arc<_> = current_tcb
        .next_task
        .upgrade()
        .expect("next task has been dropped");
    let next_tcb = next_arc.context.lock();

    println!(
        "Switching from {} ({}) to {} ({})",
        current_arc.name, current_arc.id, next_arc.name, next_arc.id
    );

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

    println!("Current sp: {cur_sp_ptr:p}");
    println!("Next sp: {next_sp_ptr:p}");
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

fn create_cyclic_task<S: Into<Cow<'static, str>>>(
    entry: extern "C" fn(),
    name: S,
) -> Arc<TaskControlBlock> {
    let _ = TASKS.is_locked(); // Force lazy init
    let _ = CURRENT_TASK.is_locked(); // Force lazy init
    println!("Locks: {} {}", TASKS.is_locked(), CURRENT_TASK.is_locked());

    let stack = KERNEL_INFO.get().unwrap().create_stack().expect("A stack");
    // // === 1) Allocate a stack ===
    // const STACK_PAGES: usize = 2;
    // let page_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    // // Pick a virtual address for the stack (for now, hardcode or use a kernel region allocator)
    // let stack_top = VirtAddr::new_truncate(0xFFFF_FF00_0000_0000);
    // let stack_bottom = stack_top - STACK_PAGES as u64 * 4096;

    // let mapper = &mut setup.page_table;
    // let frame_alloc = &mut setup.frame_allocator;

    // let pages = Page::range(
    //     Page::containing_address(stack_bottom),
    //     Page::containing_address(stack_top),
    // );

    // for page in pages {
    //     let frame = frame_alloc.allocate_frame().expect("no frames");
    //     unsafe {
    //         mapper
    //             .map_to(page, frame, page_flags, frame_alloc)
    //             .expect("map_to failed")
    //             .flush();
    //     }
    // }

    // println!("Created stack (top: {stack_top:p}; bottom: {stack_bottom:p})");

    // === 2) Prepare initial stack frame ===
    //
    // After task_switch, rsp will point to this frame and the function will `ret` into `entry`.
    let mut stack_ptr = stack.top().as_mut_ptr::<*const ()>(); // is aligned

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
    let name = name.into();

    println!("Allocated stack (sp {stack_ptr:p}) for task {name:?}");

    Arc::new_cyclic(|weak_self| {
        TaskControlBlock {
            name,
            id: uuid_v4(),
            context: Mutex::new(Context {
            stack_pointer: VirtAddr::from_ptr(stack_ptr),
            cr3: Cr3::read(),
            next_task: Weak::clone(weak_self),
            stack: Some(stack),
            dealloc: None,
            process_info: None,
        })
        }
        
    })
}

pub fn create_task(entry: extern "C" fn(), name: &'static str) {
    let tcb = create_cyclic_task(entry, name);

    // === 3) Add to task list after current ===
    {
        let mut tasks = TASKS.lock();
        println!("Locked tasks");
        let current = CURRENT_TASK.lock();
        println!("Locked current task");
        tasks.insert(tcb.clone());

        println!("Pushed tcb");

        // Fix linked list
        let mut cur_tcb = current.context.lock();
        let cur_next = cur_tcb.next_task.clone();
        cur_tcb.next_task = Arc::downgrade(&tcb);
        let mut new_tcb = tcb.context.lock();
        new_tcb.next_task = cur_next;
    }

    println!("Finished");
}

static TASK_DEALLOC: Once<Arc<TaskControlBlock>> = Once::new();

pub fn init() {
    let dealloc = create_cyclic_task(task_dealloc, "dealloc");
    let mut tasks = TASKS.lock();
    tasks.insert(dealloc.clone());
    TASK_DEALLOC.call_once(|| dealloc);
}

/// Switches to another task (and another stack) to deallloc the current task's pages
extern "C" fn task_exit() -> ! {
    println!("Ending task");
    let dealloc = TASK_DEALLOC.get().expect("Initialized dealloc");
    let current = CURRENT_TASK.lock();
    let old_next = {
        let mut current = current.context.lock();
        let old_next = current.next_task.clone();
        current.next_task = Arc::downgrade(dealloc);
        old_next
    };
    {
        let mut dealloc = dealloc.context.lock();
        dealloc.next_task = old_next;
        dealloc.dealloc = Some(current.clone());
    }
    drop(current);
    println!("Switching to dealloc");
    task_switch_safe();
    unreachable!();
}

extern "C" fn task_dealloc() {
    loop {
        if let Some(dealloc_ptr) = TASK_DEALLOC.get() {
            let mut dealloc_ptr_lock = dealloc_ptr.context.lock();
            if let Some(dealloc_task_ptr) = dealloc_ptr_lock.dealloc.take() {
                let mut dealloc_task_lock = dealloc_task_ptr.context.lock();
                println!("Cleaning up {} ({})", dealloc_task_ptr.name, dealloc_task_ptr.id);
                let mut tasks = TASKS.lock();
                tasks.remove(&dealloc_task_ptr);
                println!("Removed task from list");
                for t in tasks.iter() {
                    if t != dealloc_ptr {
                        let mut lock = t.context.lock();
                        if lock.next_task.ptr_eq(&Arc::downgrade(&dealloc_task_ptr)) {
                            println!(
                                "{} ({}) pointed to this task, rerouting to my dealloc next",
                                t.name, t.id
                            );
                            lock.next_task = dealloc_ptr_lock.next_task.clone();
                        }
                    }
                }

                if let Some(stack) = dealloc_task_lock.stack.take() {
                    let info = KERNEL_INFO.get().unwrap();
                    unsafe {
                        info.free_stack(stack);
                    }
                }
            } else {
                println!("Nothing to clean up");
            }
        } else {
            println!("Dealloc not initialized");
        }
        task_switch_safe();
    }
}

pub fn set_current_process_info(process_info: ProcessInfo) {
    CURRENT_TASK.lock().context.lock().process_info = Some(process_info)
}

#[allow(clippy::significant_drop_tightening)]
pub fn change_current_process_info<U>(f: impl Fn(&mut Option<ProcessInfo>) -> U) -> U {
    let lock = CURRENT_TASK.lock();
    let p = &mut lock.context.lock().process_info;
    f(p)
}

pub fn get_current_process_info() -> Option<ProcessInfo> {
    CURRENT_TASK.lock().context.lock().process_info.clone()
}

pub fn get_current_task_id() -> TaskId {
    if INITIALIZED.load(core::sync::atomic::Ordering::Acquire) {
        Some(CURRENT_TASK.lock().id)
    }else{
        None
    }
}


pub type TaskId = Option<Uuid>;