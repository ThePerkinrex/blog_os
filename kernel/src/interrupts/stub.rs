use x86_64::{
    VirtAddr,
    registers::{
        rflags,
        segmentation::{CS, SS, Segment},
    },
    structures::idt::InterruptStackFrame,
};

use crate::{
    gdt,
    interrupts::{int_80_handler, syscalls},
    process::get_process_kernel_stack_top,
};

const SAVED_REG_COUNT: u64 = 12; // RBP RCX, RDX, RSI, RDI, R8, R9, R10, R11, R12, R13, RAX
const SAVED_BYTES: u64 = SAVED_REG_COUNT * core::mem::size_of::<u64>() as u64;
const IRET_FRAME_BYTES: u64 = 5 * core::mem::size_of::<u64>() as u64; // SS, RSP, RFLAGS, CS, RIP
const TOTAL_FRAME_BYTES: u64 = SAVED_BYTES + IRET_FRAME_BYTES; // 15 * 8 = 120 bytes (0x78)

#[repr(C)]
struct InterruptContext {
    // Pushed by software (your pushes)
    pub rax: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbp: u64,
    // Pushed by CPU (iret frame)
    // pub rip: u64,
    // pub cs: u64,
    // pub rflags: u64,
    // pub rsp: u64,
    // pub ss: u64,
    pub frame: InterruptStackFrame,
}

#[unsafe(naked)]
extern "x86-interrupt" fn naked_interrupt_handler(_stack_frame: InterruptStackFrame) {
    core::arch::naked_asm!(
        "
        .cfi_startproc              // Start DWARF frame info
        // Save caller-saved registers
        push rbp
        mov rbp,rsp
        .cfi_adjust_cfa_offset 8
        .cfi_offset rbp, -16

        push rcx
        push rdx
        push rsi
        push rdi
        push r8
        push r9
        push r10
        push r11
        push r12
        push r13
        push rax // ESP0 stack now holds 11 registers and the 5-QWord IRET frame

        .cfi_offset rcx, -24
        .cfi_offset rdx, -32
        .cfi_offset rsi, -40
        .cfi_offset rdi, -48
        .cfi_offset r8,  -56
        .cfi_offset r9,  -64
        .cfi_offset r10, -72
        .cfi_offset r11, -80
        .cfi_offset r12, -88
        .cfi_offset r13, -96
        .cfi_offset rax, -104

        // 2. Call Rust helper to get the new Task Kernel Stack Top
        //    The return value (the new stack top) will be in RAX.
        call {get_stack_top}
        
        // RAX now holds the Task Kernel Stack Top address. 
        // We'll use R12 as the scratch register for the new RSP.
        mov r12, rax

        // 3. Perform the stack switch and copy.
        //    Calculate the new stack pointer (R12 - total frame size)
        sub r12, {frame_size}
        
        // Copy preparation: RDI=dest, RSI=source, RCX=count
        mov rdi, r12          // Destination: new RSP after switch (bottom of data)
        mov rsi, rsp          // Source: current RSP (bottom of data)
        mov rcx, {qword_count} // Total QWords to copy (13 QWords)
        
        // Set the new stack pointer *before* copying
        mov rsp, r12          // **Stack Switch occurs here!**

        // Perform the copy (13 QWords = 104 bytes)
        rep movsq             // Copy the entire stack frame and saved regs to the new stack

        
        mov rbp,rsp
        add rbp,{saved_bytes}
        sub rbp,8
        .cfi_def_cfa_register rbp

        // The stack is now the Task Kernel Stack and perfectly aligned.
        mov rdi, rsp                 // first arg: &mut InterruptContext
        mov rsi, {handler}           // second arg: ContextHandler

        // Call interrupt_entry()
        call {interrupt}

        // mov rax,rsp
        // push {zero}
        // push rax
        // pushfq                         /* push RFLAGS */
        // // call {kernel_selector}
        // //push rax                       /* push kernel CS (typical selector = 0x08) */
        // mov rax,cs
        // push rax
        // // Load RIP-relative address of naked_syscall_tail into RAX
        // lea rax, [rip + {naked_syscall_tail}]
        
        // // Push it on the stack (RIP-relative, PIE-safe)
        // push rax

        // iretq                           /* returns to kernel_resume at CPL=0 */
        
        .cfi_endproc


        // Push a new return frame for iretq for jumping to naked_syscall_tail

        // // Restore registers (reverse order)
        // pop r13
        // pop r12
        // pop r11
        // pop r10
        // pop r9
        // pop r8
        // pop rdi
        // pop rsi
        // pop rdx
        // pop rcx
        // sub rsp,8
        // pop rbp


        // // Return from interrupt — RAX still holds the return value from test_handler()
        // iretq
        ",
        interrupt = sym interrupt_handle,
        handler = sym interrupt_test_handler,
        get_stack_top = sym get_process_kernel_stack_top,
        frame_size = const TOTAL_FRAME_BYTES,
        saved_bytes = const SAVED_BYTES,
        qword_count = const (TOTAL_FRAME_BYTES / 8),
        naked_syscall_tail = sym naked_syscall_tail,
        kernel_selector = sym gdt::kernel_code_selector,
        zero = const 0u64,
    );
}

pub type ContextHandler = extern "C" fn(&mut InterruptContext);

extern "C" fn interrupt_test_handler(ctx: &mut InterruptContext) {
    ctx.rax = 0;
}

extern "C" fn interrupt_handle(ctx: &mut InterruptContext, handle: ContextHandler) -> ! {
    let rsp: u64;
    unsafe {
        core::arch::asm!("mov {reg},rsp", reg = lateout(reg) rsp);
    }
    handle(ctx);
    let cs = CS::get_reg();
    let ss = SS::get_reg();
    let rflags = rflags::read();
    let tail_frame = InterruptStackFrame::new(
        VirtAddr::from_ptr(interrupt_tail as *const u8),
        cs,
        rflags,
        VirtAddr::new_truncate(rsp),
        ss,
    );
    unsafe { core::arch::asm!("mov rdi, {frame}", frame = in(reg) ctx) }
    unsafe { tail_frame.iretq() }
}

extern "C" fn interrupt_tail(ctx: &mut InterruptContext) -> ! {
	// TODO do something


    // Restore registers
    unsafe {
        core::arch::asm!(
            // No actual instructions needed, just tell the compiler which registers to load
            "mov rbp,{rbp}",
            in("rax") ctx.rax,
            in("r13") ctx.r13,
            in("r12") ctx.r12,
            in("r11") ctx.r11,
            in("r10") ctx.r10,
            in("r9")  ctx.r9,
            in("r8")  ctx.r8,
            in("rdi") ctx.rdi,
            in("rsi") ctx.rsi,
            in("rdx") ctx.rdx,
            in("rcx") ctx.rcx,
            rbp = in(reg) ctx.rbp
        );
    }
    unsafe { ctx.frame.iretq() }
}

/// # Safety
/// Only ever reachable from naked_int_80_handler, via iretq
#[unsafe(naked)]
unsafe extern "C" fn naked_syscall_tail() {
    core::arch::naked_asm!(
        "
        .cfi_startproc
        .cfi_adjust_cfa_offset 96
        .cfi_offset rbp, -8
        .cfi_offset rcx, -16
        .cfi_offset rdx, -24
        .cfi_offset rsi, -32
        .cfi_offset rdi, -40
        .cfi_offset r8,  -48
        .cfi_offset r9,  -56
        .cfi_offset r10, -64
        .cfi_offset r11, -72
        .cfi_offset r12, -80
        .cfi_offset r13, -88
        .cfi_offset rax, -96


        call {syscall_tail}

        // Restore registers (reverse order)
        pop rax
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8
        pop rdi
        pop rsi
        pop rdx
        pop rcx
        pop rbp

        // Return from interrupt — RAX still holds the return value from test_handler()
        iretq
        
        .cfi_endproc
    ",
    syscall_tail = sym syscalls::syscall_tail
    )
}
