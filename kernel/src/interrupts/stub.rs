use x86_64::{
    VirtAddr,
    registers::{
        rflags,
        segmentation::{CS, SS, Segment},
    },
};

use crate::
    interrupts::syscalls::syscall_tail
;

// const SAVED_REG_COUNT: u64 = 10; // RBP RCX, RDX, RSI, RDI, R8, R9, R10, R11, RAX
// const SAVED_BYTES: u64 = SAVED_REG_COUNT * core::mem::size_of::<u64>() as u64;
// const IRET_FRAME_BYTES: u64 = 5 * core::mem::size_of::<u64>() as u64; // SS, RSP, RFLAGS, CS, RIP
// const TOTAL_FRAME_BYTES: u64 = SAVED_BYTES + IRET_FRAME_BYTES; // 15 * 8 = 120 bytes (0x78)

#[repr(C)]
pub struct SavedRegisters {
	pub stack_top: VirtAddr,
    pub rax: u64,
    pub rdi: u64,
	pub rbx: u64,
	pub r15: u64,
	pub r14: u64,
	pub r13: u64,
	pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbp: u64,
}

#[repr(C)]
pub struct InterruptContext {
    // Pushed by software (your pushes)
	pub registers: SavedRegisters,
    // Pushed by CPU (iret frame)
    // pub rip: u64,
    // pub cs: u64,
    // pub rflags: u64,
    // pub rsp: u64,
    // pub ss: u64,
    pub frame: x86_64::structures::idt::InterruptStackFrame,
}

macro_rules! interrupt_with_tail {
	($vis:vis extern "x86-interrupt" fn $name:ident(InterruptStackFrame) => $implementation:path) => {
		paste::paste! {
			mod [<mod_ $name>] {
				use super::*;
				#[allow(non_upper_case_globals)]
				const [<_handler_ $name>]: $crate::interrupts::stub::ContextHandler = $implementation;

				#[unsafe(naked)]
				pub extern "x86-interrupt" fn $name(_stack_frame: x86_64::structures::idt::InterruptStackFrame) {
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
						push r8
						push r9
						push r10
						push r11
						push r12
						push r13
						push r14
						push r15
						push rbx
						push rdi
						push rax // ESP0 stack now holds 11 registers and the 5-QWord IRET frame

						// CFI register offsets
						.cfi_adjust_cfa_offset 112  // 14 registers * 8 bytes
						.cfi_offset rcx, -24
						.cfi_offset rdx, -32
						.cfi_offset rsi, -40
						.cfi_offset r8,  -48
						.cfi_offset r9,  -56
						.cfi_offset r10, -64
						.cfi_offset r11, -72
						.cfi_offset r12, -80
						.cfi_offset r13, -88
						.cfi_offset r14, -96
						.cfi_offset r15, -104
						.cfi_offset rbx, -112
						.cfi_offset rdi, -120
						.cfi_offset rax, -128

						// 2. Call Rust helper to get the new Task Kernel Stack Top
						//    The return value (the new stack top) will be in RAX.
						call {get_stack_top}
						push rax // stack top


						// if RAX is 0, no stack switch must happen, jump to the handler
						test rax, rax
						jz  .no_stack_switch
						
						// RAX now holds the Task Kernel Stack Top address. 

						// 3. Perform the stack switch and copy.
						//    Calculate the new stack pointer (RAX - total frame size)
						sub rax, {frame_size}
						
						// Copy preparation: RDI=dest, RSI=source, RCX=count
						mov rdi, rax          // Destination: new RSP after switch (bottom of data)
						mov rsi, rsp          // Source: current RSP (bottom of data)
						mov rcx, {qword_count} // Total QWords to copy (13 QWords)
						
						// Set the new stack pointer *before* copying
						mov rsp, rax          // **Stack Switch occurs here!**

						// Perform the copy (13 QWords = 104 bytes)
						rep movsq             // Copy the entire stack frame and saved regs to the new stack

					.no_stack_switch:
						
						mov rbp,rsp
						add rbp,{saved_bytes}
						sub rbp,8
						.cfi_def_cfa_register rbp

						// The stack is now the Task Kernel Stack and perfectly aligned.
						mov rdi, rsp                 // first arg: &mut InterruptContext
						lea rsi, [rip + {handler}]   // second arg: ContextHandler

						// Call interrupt_entry()
						call {interrupt}
						
						.cfi_endproc
						",
						interrupt = sym $crate::interrupts::stub::interrupt_handle,
						handler = sym $implementation,
						get_stack_top = sym $crate::process::get_task_kernel_stack_top,
						frame_size = const core::mem::size_of::<$crate::interrupts::stub::InterruptContext>(),
						saved_bytes = const core::mem::size_of::<$crate::interrupts::stub::SavedRegisters>(),
						qword_count = const (core::mem::size_of::<$crate::interrupts::stub::InterruptContext>() / 8),
					);
				}
			}
			$vis use [<mod_ $name>]::$name;
		}
	};
}

// #[unsafe(naked)]
// extern "x86-interrupt" fn naked_interrupt_handler(_stack_frame: x86_64::structures::idt::InterruptStackFrame) {
//     core::arch::naked_asm!(
//         "
//         .cfi_startproc              // Start DWARF frame info
//         // Save caller-saved registers
//         push rbp
//         mov rbp,rsp
//         .cfi_adjust_cfa_offset 8
//         .cfi_offset rbp, -16

//         push rcx
//         push rdx
//         push rsi
//         push r8
//         push r9
//         push r10
//         push r11
//         push r12
//         push r13
//         push r14
//         push r15
//         push rbx
//         push rdi
//         push rax // ESP0 stack now holds 11 registers and the 5-QWord IRET frame

//         // CFI register offsets
// 		.cfi_adjust_cfa_offset 112  // 14 registers * 8 bytes
//         .cfi_offset rcx, -24
//         .cfi_offset rdx, -32
//         .cfi_offset rsi, -40
//         .cfi_offset r8,  -48
//         .cfi_offset r9,  -56
//         .cfi_offset r10, -64
//         .cfi_offset r11, -72
//         .cfi_offset r12, -80
//         .cfi_offset r13, -88
//         .cfi_offset r14, -96
//         .cfi_offset r15, -104
//         .cfi_offset rbx, -112
//         .cfi_offset rdi, -120
//         .cfi_offset rax, -128

//         // 2. Call Rust helper to get the new Task Kernel Stack Top
//         //    The return value (the new stack top) will be in RAX.
//         call {get_stack_top}
// 		push rax // stack top


// 		// if RAX is 0, no stack switch must happen, jump to the handler
// 		test rax, rax
// 		jz  .no_stack_switch
        
//         // RAX now holds the Task Kernel Stack Top address. 

//         // 3. Perform the stack switch and copy.
//         //    Calculate the new stack pointer (RAX - total frame size)
//         sub rax, {frame_size}
        
//         // Copy preparation: RDI=dest, RSI=source, RCX=count
//         mov rdi, rax          // Destination: new RSP after switch (bottom of data)
//         mov rsi, rsp          // Source: current RSP (bottom of data)
//         mov rcx, {qword_count} // Total QWords to copy (13 QWords)
        
//         // Set the new stack pointer *before* copying
//         mov rsp, rax          // **Stack Switch occurs here!**

//         // Perform the copy (13 QWords = 104 bytes)
//         rep movsq             // Copy the entire stack frame and saved regs to the new stack

// .no_stack_switch:
        
//         mov rbp,rsp
//         add rbp,{saved_bytes}
//         sub rbp,8
//         .cfi_def_cfa_register rbp

//         // The stack is now the Task Kernel Stack and perfectly aligned.
//         mov rdi, rsp                 // first arg: &mut InterruptContext
//         mov rsi, {handler}           // second arg: ContextHandler

//         // Call interrupt_entry()
//         call {interrupt}
        
//         .cfi_endproc
//         ",
//         interrupt = sym interrupt_handle,
//         handler = sym interrupt_test_handler,
//         get_stack_top = sym crate::process::get_process_kernel_stack_top,
//         frame_size = const core::mem::size_of::<InterruptContext>(),
//         saved_bytes = const core::mem::size_of::<SavedRegisters>(),
//         qword_count = const (core::mem::size_of::<InterruptContext>() / 8),
//     );
// }

pub type ContextHandler = extern "C" fn(&mut InterruptContext);

// extern "C" fn interrupt_test_handler(ctx: &mut InterruptContext) {
//     ctx.registers.rax = 0;
// }

pub extern "C" fn interrupt_handle(ctx: &mut InterruptContext, handle: ContextHandler) -> ! {
    let rsp: u64;
    unsafe {
        core::arch::asm!("mov {reg},rsp", reg = lateout(reg) rsp);
    }
    handle(ctx);
    let cs = CS::get_reg();
    let ss = SS::get_reg();
    let rflags = rflags::read();
    let tail_frame = x86_64::structures::idt::InterruptStackFrame::new(
        VirtAddr::from_ptr(interrupt_tail as *const u8),
        cs,
        rflags,
        VirtAddr::new_truncate(rsp),
        ss,
    );
    // unsafe { core::arch::asm!("mov rdi, {frame}", frame = in(reg) ctx) }
    // unsafe { tail_frame.iretq() }
	unsafe {
        core::arch::asm!(
            // Restore registers (reverse order not required for movs)
			// Pushes before movs because it may use registers needed for the movs
			"push {frame_ss:r}",
			"push {frame_sp}",
			"push {frame_rflags}",
			"push {frame_cs:r}",
			"push {frame_ip}",
            // "mov rdi, rdi", rdi should be ctx 
			"iretq",

            // ctx.registers base
            in("rdi") ctx,
			frame_ss = in(reg) tail_frame.stack_segment.0,
			frame_sp = in(reg) tail_frame.stack_pointer.as_u64(),
			frame_rflags = in(reg) tail_frame.cpu_flags.bits(),
			frame_cs = in(reg) tail_frame.code_segment.0,
			frame_ip = in(reg) tail_frame.instruction_pointer.as_u64(),
			options(noreturn)
        );
    }
}

pub extern "C" fn interrupt_tail(ctx: &mut InterruptContext) -> ! {
	if !ctx.registers.stack_top.is_null() { // If we didnt change stacks, there was no task stack - no multitasking possible
		syscall_tail();
	}


    // Restore registers
    unsafe {
        core::arch::asm!(
            // Restore registers (reverse order not required for movs)
			// Pushes before movs because it may use registers needed for the movs
			"push {frame_ss:r}",
			"push {frame_sp}",
			"push {frame_rflags}",
			"push {frame_cs:r}",
			"push {frame_ip}",
            "mov rax, [rdi + 8*1]",
            "mov rbx, [rdi + 8*3]",
            "mov r15, [rdi + 8*4]",
            "mov r14, [rdi + 8*5]",
            "mov r13, [rdi + 8*6]",
            "mov r12, [rdi + 8*7]",
            "mov r11, [rdi + 8*8]",
            "mov r10, [rdi + 8*9]",
            "mov r9,  [rdi + 8*10]",
            "mov r8,  [rdi + 8*11]",
            "mov rsi, [rdi + 8*12]",
            "mov rdx, [rdi + 8*13]",
            "mov rcx, [rdi + 8*14]",
            "mov rbp, [rdi + 8*15]",
            "mov rdi, [rdi + 8*2]", // as were using rdi, restore it last
			"iretq",

            // ctx.registers base
            in("rdi") &ctx.registers,
			frame_ss = in(reg) ctx.frame.stack_segment.0,
			frame_sp = in(reg) ctx.frame.stack_pointer.as_u64(),
			frame_rflags = in(reg) ctx.frame.cpu_flags.bits(),
			frame_cs = in(reg) ctx.frame.code_segment.0,
			frame_ip = in(reg) ctx.frame.instruction_pointer.as_u64(),
			options(noreturn)
        );
    }
}

