use core::borrow::Borrow;

use x86_64::{VirtAddr, instructions::interrupts, structures::gdt::SegmentSelector};

use crate::{elf::LoadedProgram, gdt::selectors};

pub fn jmp_to_usermode<T: Borrow<LoadedProgram>>(prog: T) {
    let selectors = selectors();
    let prog_ref = prog.borrow();
    let entry = prog_ref.entry();
    let stack_top = prog_ref.stack().top();

    drop(prog);

    unsafe {
        jump_to_ring3(
            entry,
            stack_top,
            selectors.user_data_selector,
            selectors.user_code_selector,
        );
    }
}

unsafe fn jump_to_ring3(
    entry: VirtAddr,
    stack_top: VirtAddr,
    user_data: SegmentSelector,
    user_code: SegmentSelector,
) -> ! {
    use core::arch::asm;

    interrupts::disable(); // Interrupts disabled during the switch
    unsafe {
        asm!(
            "push {user_data}",     // SS
            "push {stack}",         // RSP
            "pushfq",               // RFLAGS
            "pop rax",
            "or rax, 0x200",        // IF=1
            "push rax",
            "push {user_code}",     // CS
            "push {entry}",         // RIP
            "iretq",
            user_data = in(reg) u64::from(user_data.0),
            user_code = in(reg) u64::from(user_code.0),
            stack = in(reg) stack_top.as_u64(),
            entry = in(reg) entry.as_u64(),
            options(noreturn),
        );
    }
}

// extern "C" fn test_usermode() {
//     println!("IN USERMODE");
//     interrupts::disable(); // GP
//     println!("Still in usemode");
//     hlt_loop()
// }
