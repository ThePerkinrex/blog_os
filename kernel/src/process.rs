use core::ops::DerefMut;

use alloc::sync::Arc;
use x86_64::VirtAddr;

use crate::{KERNEL_INFO, elf::LoadedProgram, gdt::get_esp0_stack_top, stack::SlabStack};

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

    fn get_kernel_stack(&mut self) -> &Arc<SlabStack> {
        self.kernel_stack.get_or_insert_with(|| {
            let stack = KERNEL_INFO
                .get()
                .unwrap()
                .lock()
                .create_stack()
                .expect("A stack");
            Arc::new(stack)
        })
    }

    /// # Safety
    /// We must be on ESP0
    /// No references to elements in the stack must be held across this call
    pub unsafe fn swap_to_kernel_stack(&mut self) {
        let new_stack_top = self.get_kernel_stack().top();
        let old_stack_top = get_esp0_stack_top();

        let old_sp: u64;

        unsafe { core::arch::asm!("mov {0},rsp", out(reg) old_sp) }

        let old_sp = VirtAddr::new(old_sp);

        assert!(old_sp < old_stack_top);

        let old_sp_size = old_stack_top - old_sp;

        let new_sp = new_stack_top - old_sp_size;

        {
            // VERY BAD -> RBP is not changed
            let old_sp_data =
                unsafe { core::slice::from_raw_parts(old_sp.as_ptr::<u8>(), old_sp_size as usize) };
            let new_sp_data = unsafe {
                core::slice::from_raw_parts_mut(new_sp.as_mut_ptr::<u8>(), old_sp_size as usize)
            };
            new_sp_data.copy_from_slice(old_sp_data);
        }

        let new_sp = new_sp.as_u64();

        // Data is copied, now just set the new stack pointer
        unsafe {
            core::arch::asm!("mov rsp,{0}", in(reg) new_sp);
        }
    }
}
