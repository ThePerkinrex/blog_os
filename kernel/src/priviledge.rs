use core::ops::DerefMut;

use x86_64::{VirtAddr, instructions::interrupts, structures::gdt::SegmentSelector};

use crate::{KERNEL_INFO, elf::LoadedProgram, gdt::selectors, hlt_loop, println};

pub fn test_jmp_to_usermode(prog: LoadedProgram) {
    let selectors = selectors();

    unsafe {
        jump_to_ring3(
            prog.entry(),
            prog.stack().top(),
            selectors.user_data_selector,
            selectors.user_code_selector,
        );
    } // Crashes - not on a user accessible page
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
