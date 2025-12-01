use core::{
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

use alloc::sync::Arc;
use blog_os_vfs::api::{
    IOError,
    file::{File, cglue_file::FileBox},
    inode::INode,
    path::Path,
};
use kernel_utils::{aligned_bytes::AlignedBytes, simple_slotmap::SimpleSlotmap};
use log::{debug, info, warn};
use shared_fs::FileType;
use spin::lock_api::RwLock;
use thiserror::Error;

use crate::{
    KERNEL_INFO,
    elf::{ElfHeader, ElfLoadError, LoadedProgram, load_user_program},
    fs::VFS,
    memory::multi_l4_paging::PageTableToken,
    multitask::{get_current_task, set_current_process_info},
    priviledge::jmp_to_usermode,
    rand::uuid_v4,
};

pub mod stdio;

#[derive(Debug, Clone, Default)]
pub enum ProcessStatus {
    #[default]
    Ok,
    Ending(u64),
}

// #[derive(Debug, Clone)]
// pub struct Stdout {
//     buf: String,
// }

// impl Stdout {
//     pub fn write(&mut self, data: &str) -> usize {
//         let combined = core::mem::take(&mut self.buf) + data;

//         let lines = combined.split_inclusive('\n');

//         let mut sum = 0;
//         for line in lines {
//             sum += line.len();
//             if line.ends_with('\n') {
//                 info!("[STDOUT] {}", line.trim_end_matches('\n'));
//             } else {
//                 self.buf += line;
//             }
//         }

//         sum
//     }

//     pub fn flush(&mut self) {
//         let data = core::mem::take(&mut self.buf);
//         if !data.is_empty() {
//             info!("[FLUSHED] [STDOUT] {data}");
//         }
//     }
// }

pub struct OpenFile {
    file: ManuallyDrop<FileBox<'static>>,
}

impl From<FileBox<'static>> for OpenFile {
    fn from(file: FileBox<'static>) -> Self {
        Self {
            file: ManuallyDrop::new(file),
        }
    }
}

impl Deref for OpenFile {
    type Target = FileBox<'static>;

    fn deref(&self) -> &Self::Target {
        &self.file
    }
}

impl DerefMut for OpenFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.file
    }
}

impl Drop for OpenFile {
    fn drop(&mut self) {
        let _ = self.file.close();
    }
}

pub struct ProcessInfo {
    program: Arc<LoadedProgram>,
    status: ProcessStatus,
    id: uuid::Uuid,
    original: uuid::Uuid,
    pt_token: Option<Arc<PageTableToken>>,
    // stdout: Stdout,
    files: Arc<RwLock<SimpleSlotmap<Arc<RwLock<OpenFile>>>>>,
}

impl core::fmt::Debug for ProcessInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ProcessInfo")
            .field("program", &self.program)
            .field("status", &self.status)
            .field("id", &self.id)
            .field("original", &self.original)
            .field("pt_token", &self.pt_token)
            .field("files_len", &self.files.read().len())
            .finish()
    }
}

impl Clone for ProcessInfo {
    fn clone(&self) -> Self {
        let id = uuid_v4();
        // let refs = Arc::strong_count(&self.program);
        // debug!(
        //     "CLONING PROCESS INFO {} -> {id} (refs: {} -> {})",
        //     self.id,
        //     refs,
        //     refs + 1
        // );
        // unwind::backtrace();
        Self {
            program: self.program.clone(),
            status: self.status.clone(),
            id,
            original: self.original,
            pt_token: self.pt_token.clone(),
            files: self.files.clone(),
        }
    }
}

impl Drop for ProcessInfo {
    fn drop(&mut self) {
        // let refs = Arc::strong_count(self.program());
        // debug!(
        //     "Dropping {} ({}) (refs {refs} -> {})",
        //     self.id,
        //     self.original,
        //     refs - 1
        // )
    }
}

static FIRST_PROC: AtomicBool = AtomicBool::new(true);

impl ProcessInfo {
    pub fn new(prog: &[u8]) -> Result<Self, ElfLoadError> {
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
        let prog = load_user_program(prog)?;
        info!("[{id}] Loaded elf");

        Ok(Self {
            program: Arc::new(prog),
            status: ProcessStatus::default(),
            id,
            original: id,
            pt_token: token,
            files: Default::default(),
        })
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

    pub const fn process_id(&self) -> uuid::Uuid {
        self.original
    }

    pub const fn info_id(&self) -> uuid::Uuid {
        self.id
    }

    pub const fn files(&self) -> &Arc<RwLock<SimpleSlotmap<Arc<RwLock<OpenFile>>>>> {
        &self.files
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

#[derive(Debug, Error)]
pub enum ExecError {
    #[error(transparent)]
    Elf(#[from] ElfLoadError),

    #[error(transparent)]
    Io(#[from] IOError),

    #[error("Not a regular file")]
    NotRegularFile,
}

pub fn load(path: &Path) -> Result<ProcessInfo, ExecError> {
    let inode = VFS.write().get(path)?;

    let stat = inode.stat()?;

    if stat.file_type != FileType::RegularFile {
        warn!("Tried to load {path} with stat: {stat:?}");
        return Err(ExecError::NotRegularFile);
    }

    let mut buf = AlignedBytes::new_uninit::<ElfHeader>(stat.size as usize);

    let mut file = inode.open()?;

    let mut read = &mut *buf;

    while !read.is_empty() {
        let bytes = file.read(read)?;
        if let Some(r) = read.get_mut(bytes..) {
            read = r;
        } else {
            break;
        }
    }

    file.close()?;

    debug!("Loaded {path}");

    Ok(ProcessInfo::new(&buf)?)
}
