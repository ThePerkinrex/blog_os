use core::sync::atomic::{AtomicBool, Ordering};

use alloc::sync::Arc;

use crate::{
    KERNEL_INFO,
    elf::{LoadedProgram, load_elf},
    gdt::get_esp0_stack_top,
    multitask::{change_current_process_info, set_current_process_info},
    println,
    priviledge::jmp_to_usermode,
    stack::SlabStack,
};

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    program: Arc<LoadedProgram>,
    kernel_stack: Option<Arc<SlabStack>>,
}

static FIRST_PROC: AtomicBool = AtomicBool::new(true);

impl ProcessInfo {
    pub fn new(prog: &[u8]) -> Self {
        if FIRST_PROC
            .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            println!("Not first proc, creating a new l4 table");
            println!("Before CR3: {:?}", x86_64::registers::control::Cr3::read());

            KERNEL_INFO
                .get()
                .unwrap()
                .create_p4_table_and_switch();

            println!("CR3: {:?}", x86_64::registers::control::Cr3::read());
        } else {
            println!("Creating first proc, not creating a new l4 table");
        }

        
        println!("Loading elf");
        let prog = load_elf(prog);
        println!("Loaded elf");

        Self {
            program: Arc::new(prog),
            kernel_stack: None,
        }
    }

    pub fn start(self) {
        println!("Starting process");
        let prog = self.program.clone();
        set_current_process_info(self);
        jmp_to_usermode(&prog);
    }

    fn get_kernel_stack(&mut self) -> &Arc<SlabStack> {
        self.kernel_stack.get_or_insert_with(|| {
            let stack = KERNEL_INFO
                .get()
                .unwrap()
                .create_stack()
                .expect("A stack");
            println!("Created a new stack for the process: {stack:?}");
            Arc::new(stack)
        })
    }
}

pub extern "C" fn get_process_kernel_stack_top() -> u64 {
    change_current_process_info(|pi| {
        let top = pi.as_mut().map_or_else(
            || {
                println!("[WARN] No current process, returning esp0 top");
                get_esp0_stack_top()
            },
            |pi| pi.get_kernel_stack().top(),
        );
        println!("Returning stack top: {top:p}");
        top
    })
    .as_u64()
}
