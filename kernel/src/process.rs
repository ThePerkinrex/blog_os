use core::sync::atomic::{AtomicBool, Ordering};

use alloc::{string::String, sync::Arc};
use log::{debug, info};

use crate::{
    KERNEL_INFO,
    elf::{LoadedProgram, load_elf},
    memory::multi_l4_paging::PageTableToken,
    multitask::{get_current_task, get_current_task_id, set_current_process_info},
    priviledge::jmp_to_usermode,
    rand::uuid_v4,
    unwind,
};

#[derive(Debug, Clone, Default)]
pub enum ProcessStatus {
    #[default]
    Ok,
    Ending(u64),
}

#[derive(Debug, Clone)]
pub struct Stdout {
    buf: String,
}

impl Stdout {
    pub fn write(&mut self, data: &str) -> usize {
        let combined = core::mem::take(&mut self.buf) + data;

        let lines = combined.split_inclusive('\n');


        
        let task_id = get_current_task_id();

        let mut sum = 0;
        for line in lines {
            sum += line.len();
            if line.ends_with('\n') {
                info!("[{task_id:?}] [STDOUT] {}", line.trim_end_matches('\n'));
            }else{
                self.buf += line;
            }
        }

        sum
        
    }

    pub fn flush(&mut self) {
        let task_id = get_current_task_id();
        let data = core::mem::take(&mut self.buf);
        
        info!("[{task_id:?}] [FLUSHED] [STDOUT] {data}");
    }
}

#[derive(Debug)]
pub struct ProcessInfo {
    program: Arc<LoadedProgram>,
    status: ProcessStatus,
    id: uuid::Uuid,
    original: uuid::Uuid,
    pt_token: Option<Arc<PageTableToken>>,
    stdout: Stdout
}

impl Clone for ProcessInfo {
    fn clone(&self) -> Self {
        let id = uuid_v4();
        let refs = Arc::strong_count(&self.program);
        debug!(
            "CLONING PROCESS INFO {} -> {id} (refs: {} -> {})",
            self.id,
            refs,
            refs + 1
        );
        unwind::backtrace();
        Self {
            program: self.program.clone(),
            status: self.status.clone(),
            id,
            original: self.original,
            pt_token: self.pt_token.clone(),
            stdout: self.stdout.clone()
        }
    }
}

impl Drop for ProcessInfo {
    fn drop(&mut self) {
        let refs = Arc::strong_count(self.program());
        debug!(
            "Dropping {} ({}) (refs {refs} -> {})",
            self.id,
            self.original,
            refs - 1
        )
    }
}

static FIRST_PROC: AtomicBool = AtomicBool::new(true);

impl ProcessInfo {
    pub fn new(prog: &[u8]) -> Self {
        let id = uuid_v4();
        let token;
        if FIRST_PROC
            .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            info!("[{id}] Not first proc, creating a new l4 table");
            debug!(
                "[{id}] Before CR3: {:?}",
                x86_64::registers::control::Cr3::read()
            );

            token = Some(KERNEL_INFO.get().unwrap().create_p4_table_and_switch());

            info!("[{id}] CR3: {:?}", x86_64::registers::control::Cr3::read());
        } else {
            info!("[{id}] Creating first proc, not creating a new l4 table");
            token = None;
        }

        debug!("[{id}] Loading elf");
        let prog = load_elf(prog);
        info!("[{id}] Loaded elf");

        Self {
            program: Arc::new(prog),
            status: ProcessStatus::default(),
            id,
            original: id,
            pt_token: token,
            stdout: Stdout { buf: String::new() }
        }
    }

    pub fn start(self) {
        info!(
            "[{} from {}] Starting process (refs: {})",
            self.id,
            self.original,
            Arc::strong_count(&self.program)
        );
        let prog = self.program.clone();
        set_current_process_info(self);
        jmp_to_usermode(prog);
    }

    // fn get_kernel_stack(&mut self) -> &Arc<SlabStack> {
    //     self.kernel_stack.get_or_insert_with(|| {
    //         get_current_task().
    //         let stack = KERNEL_INFO.get().unwrap().create_stack().expect("A stack");
    //         info!("Created a new stack for the process: {stack:?}");
    //         Arc::new(stack)
    //     })
    // }

    pub fn program(&self) -> &Arc<LoadedProgram> {
        info!(
            "[{} from {}] Getting program (refs: {})",
            self.id,
            self.original,
            Arc::strong_count(&self.program)
        );

        &self.program
    }

    pub const fn status(&self) -> &ProcessStatus {
        &self.status
    }

    pub const fn status_mut(&mut self) -> &mut ProcessStatus {
        &mut self.status
    }
    
    pub const fn stdout_mut(&mut self) -> &mut Stdout {
        &mut self.stdout
    }
}

/// Uses the task stack if possible
pub extern "C" fn get_process_kernel_stack_top() -> u64 {
    let tcb = get_current_task();

    let mut ctx = tcb.context.lock();

    let stack = ctx.stack.get_or_insert_with(|| {
        let stack = KERNEL_INFO.get().unwrap().create_stack().expect("A stack");
        info!("Created a new stack for the process: {stack:?}");
        stack
    });

    let stack = stack.top();

    drop(ctx);

    stack.as_u64()
}
