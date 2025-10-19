use core::ops::DerefMut;

use alloc::sync::Arc;
use x86_64::VirtAddr;

use crate::{elf::LoadedProgram, gdt::get_esp0_stack_top, multitask::{change_current_process_info, get_current_process_info, set_current_process_info}, println, priviledge::jmp_to_usermode, stack::SlabStack, KERNEL_INFO};

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    program: Arc<LoadedProgram>,
    kernel_stack: Option<Arc<SlabStack>>,
}

impl ProcessInfo {
    pub fn new(prog: LoadedProgram) -> Self {
        Self {
            program: Arc::new(prog),
            kernel_stack: None,
        }
    }

    pub fn start(self) {
        let prog = self.program.clone();
        set_current_process_info(self);
        jmp_to_usermode(&prog);
    }

    fn get_kernel_stack(&mut self) -> &Arc<SlabStack> {
        self.kernel_stack.get_or_insert_with(|| {
            let stack = KERNEL_INFO
                .get()
                .unwrap()
                .lock()
                .create_stack()
                .expect("A stack");
            println!("Created a new stack for the process: {stack:?}");
            Arc::new(stack)
        })
    }
}

pub extern "C" fn get_process_kernel_stack_top() -> u64 {
    change_current_process_info(|pi| {
        pi.as_mut().expect("A process").get_kernel_stack().top()
    }).as_u64()
}
