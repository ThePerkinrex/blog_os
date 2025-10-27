use pic8259::ChainedPics;
use spin::Lazy;
use x86_64::{
    VirtAddr,
    instructions::port::Port,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};

use crate::{
    setup::KERNEL_INFO, gdt, hlt_loop, interrupts, multitask, print, println,
    process::get_process_kernel_stack_top, test_return,
};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

mod syscalls;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

pub fn init_pics() {
    // init_timer(1);
    unsafe {
        PICS.lock().initialize();
    }

    // The Master PIC's Interrupt Mask Register (IMR) is at port 0x21.
    let mut master_mask_port = Port::<u8>::new(0x21);

    unsafe {
        // Read the current mask
        let current_mask = master_mask_port.read();

        // Clear the lowest bit (IRQ 0/Timer)
        // 0b11111110 (0xFE) ensures only IRQ 0 is unmasked, keeping others as they were.
        master_mask_port.write(current_mask & 0b11111100);
    }

    x86_64::instructions::interrupts::enable();
}
// Function to initialize the PIT to generate interrupts at a specific frequency
// pub fn init_timer(frequency_hz: u32) {
//     // The PIT base frequency is 1,193,182 Hz
//     const PIT_BASE_FREQ: u32 = 1193182;

//     // Calculate divisor
//     let divisor: u16 = (PIT_BASE_FREQ / frequency_hz) as u16;

//     // PIT Ports
//     const PIT_CMD_PORT: u16 = 0x43;
//     const PIT_DATA_PORT: u16 = 0x40; // Channel 0 (Timer)

//     // Command: Channel 0, LOBYTE/HIBYTE, Mode 3 (Square Wave Generator)
//     const COMMAND: u8 = 0b00110110;

//     // Create mutable port instances
//     let mut cmd_port = Port::<u8>::new(PIT_CMD_PORT);
//     let mut data_port = Port::<u8>::new(PIT_DATA_PORT);

//     unsafe {
//         // 1. Send the command byte
//         cmd_port.write(COMMAND);

//         // 2. Send divisor (Low byte then High byte)
//         data_port.write((divisor & 0xFF) as u8);
//         data_port.write((divisor >> 8) as u8);
//     }
// }

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    const fn as_u8(self) -> u8 {
        self as u8
    }
}

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);
    unsafe {
        idt.general_protection_fault
            .set_handler_fn(general_protection_fault_handler)
            .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX); // new
    }
    idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
    idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);
    idt[0x80]
        .set_handler_fn(naked_int_80_handler)
        .set_privilege_level(x86_64::PrivilegeLevel::Ring3);
    idt
});

pub fn init_idt() {
    IDT.load();
}

const SAVED_REG_COUNT: u64 = 13; // RBP ONE RCX, RDX, RSI, RDI, R8, R9, R10, R11, R12, R13, RAX
const SAVED_BYTES: u64 = SAVED_REG_COUNT * core::mem::size_of::<u64>() as u64;
const IRET_FRAME_BYTES: u64 = 5 * core::mem::size_of::<u64>() as u64; // SS, RSP, RFLAGS, CS, RIP
const TOTAL_FRAME_BYTES: u64 = SAVED_BYTES + IRET_FRAME_BYTES; // 15 * 8 = 120 bytes (0x78)

#[unsafe(naked)]
extern "x86-interrupt" fn naked_int_80_handler(_stack_frame: InterruptStackFrame) {
    core::arch::naked_asm!(
        "
        // Save caller-saved registers
        push rbp
        push {one}
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
        sub rbp,24

        // The stack is now the Task Kernel Stack and perfectly aligned.
        pop rax

        mov rdi,[rsp+48]
        mov rsi,[rsp+56]
        mov rdx,[rsp+64]
        mov r10,[rsp+24]
        mov r8,[rsp+40]
        mov r9,[rsp+32]

        // Call test_handler()
        call {handler}

        push rax


        mov rax,rsp
        push {zero}
        push rax
        pushfq                         /* push RFLAGS */
        // call {kernel_selector}
        //push rax                       /* push kernel CS (typical selector = 0x08) */
        mov rax,cs
        push rax
        // Load RIP-relative address of naked_syscall_tail into RAX
        lea rax, [rip + {naked_syscall_tail}]
        
        // Push it on the stack (RIP-relative, PIE-safe)
        push rax

        iretq                           /* returns to kernel_resume at CPL=0 */


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
        handler = sym int_80_handler,
        get_stack_top = sym get_process_kernel_stack_top,
        frame_size = const TOTAL_FRAME_BYTES,
        saved_bytes = const SAVED_BYTES,
        qword_count = const (TOTAL_FRAME_BYTES / 8),
        naked_syscall_tail = sym naked_syscall_tail,
        kernel_selector = sym gdt::kernel_code_selector,
        zero = const 0u64,
        one = const 1u64
    );
}

#[unsafe(naked)]
extern "C" fn naked_syscall_tail() {
    core::arch::naked_asm!(
        "
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
        add rsp,8
        pop rbp

        // Return from interrupt — RAX still holds the return value from test_handler()
        iretq
    ",
    syscall_tail = sym interrupts::syscalls::syscall_tail
    )
}

extern "C" fn int_80_handler() -> u64 {
    let code: u64;
    let arg1: u64;
    let arg2: u64;
    let arg3: u64;
    let arg4: u64;
    let arg5: u64;
    let arg6: u64;
    unsafe {
        core::arch::asm!(
            "",
            out("rdi") arg1,
            out("rsi") arg2,
            out("rdx") arg3,
            out("r10") arg4,
            out("r8") arg5,
            out("r9") arg6,
            out("rax") code,
            options(nostack, preserves_flags, nomem)
        )
    }
    // Move to task kernel stack
    syscalls::syscall_handle(code, arg1, arg2, arg3, arg4, arg5, arg6)
}

// extern "x86-interrupt" fn int_80_handler(stack_frame: InterruptStackFrame) {
//     let code: u64;
//     let arg1: u64;
//     let arg2: u64;
//     let arg3: u64;
//     let arg4: u64;
//     let arg5: u64;
//     let arg6: u64;
//     unsafe {
//         core::arch::asm!(
//             "",
//             out("rdi") arg1,
//             out("rsi") arg2,
//             out("rdx") arg3,
//             out("r10") arg4,
//             out("r8") arg5,
//             out("r9") arg6,
//             out("rax") code,
//             options(nostack, preserves_flags, nomem)

//         )
//     }
//     let res = syscalls::syscall_handle(code, arg1, arg2, arg3, arg4, arg5, arg6);
//     println!("Syscall res: {res}");

// }

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    let current_task_arc = multitask::get_current_task();
    let current_task = current_task_arc.context.lock();

    let addr = Cr2::read();

    println!("EXCEPTION: PAGE FAULT");
    println!("Current task: {:?} ({})", current_task_arc.name, current_task_arc.id);
    println!("Current stack: {:?}", current_task.stack);
    if let Ok(addr) = addr {
        if let Some(s) = &current_task.stack {
            let kinf = KERNEL_INFO.get().unwrap();
            let stack_alloc = &kinf.stack_alloc;
            if stack_alloc.is_locked() {
                unsafe {
                    stack_alloc.force_unlock();
                }
            }
            // CRITICAL SITUATION, not going back
            match stack_alloc.lock().detect_guard_page_access(addr, s) {
                crate::stack::GuardPageInfo::CurrentStackOverflow => {
                    println!("[KSTACKS] Current stack overflow")
                }
                crate::stack::GuardPageInfo::CurrentStack => {
                    println!("[KSTACKS] Current stack access")
                }
                crate::stack::GuardPageInfo::OtherGuardPage(idx, alloc) => {
                    println!("[KSTACKS] Other stack overflow (idx: {idx}, alloc: {alloc})")
                }
                crate::stack::GuardPageInfo::OtherStack(idx, alloc) => {
                    println!("[KSTACKS] Other stack access (idx: {idx}, alloc: {alloc})")
                }
                crate::stack::GuardPageInfo::Unknown => println!("[KSTACKS] Unknown access"),
            }
        } else {
            println!(
                "[KSTACKS] Addr was err or stack was not present: {addr:?}, {:?}",
                current_task.stack
            )
        }
        if let Some(pinf) = &current_task.process_info {
            println!("[PROCESS] {pinf:#?}")
        }
    }

    println!("Accessed Address: {:?}", addr);
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    drop(current_task);
    hlt_loop();
}

#[repr(u8)]
#[derive(Debug)]
enum SelectorTableCode {
    Gdt = 0b00,
    Idt = 0b01,
    Ldt = 0b10,
    Idt2 = 0b11,
}

#[derive(Debug)]
struct SelectorErrorCode {
    #[allow(unused)]
    idx: u16,
    #[allow(unused)]
    tbl: SelectorTableCode,
    #[allow(unused)]
    external: u8,
}

impl SelectorErrorCode {
    fn new(code: u64) -> Self {
        let external = (code & 0b1) as u8;
        let tbl_code = ((code >> 1) & 0b11) as u8;
        let tbl = match tbl_code {
            0b00 => SelectorTableCode::Gdt,
            0b01 => SelectorTableCode::Idt,
            0b10 => SelectorTableCode::Ldt,
            0b11 => SelectorTableCode::Idt2,
            _ => unreachable!(),
        };
        let idx = ((code >> 3) & 0b1_1111_1111_1111) as u16;
        Self { external, idx, tbl }
    }
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    let code = if error_code == 0 {
        None
    } else {
        Some(SelectorErrorCode::new(error_code))
    };
    panic!(
        "EXCEPTION: GENERAL PROTECTION FAULT ({error_code:x} = {code:?})\n{:#?}",
        stack_frame
    );
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    print!(".");
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
    // println!("Finished timer, waiting");
    // hlt_loop();
}

extern "x86-interrupt" fn keyboard_interrupt_handler(mut stack_frame: InterruptStackFrame) {
    use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    static KEYBOARD: Lazy<Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>>> = Lazy::new(|| {
        Mutex::new(Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::Ignore,
        ))
    });

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);

    let scancode: u8 = unsafe { port.read() };
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode)
        && let Some(key) = keyboard.process_keyevent(key_event)
    {
        match key {
            DecodedKey::Unicode(character) => print!("{}", character),
            DecodedKey::RawKey(key) => print!("{:?}", key),
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
    let addr = VirtAddr::from_ptr(test_return as *const ());

    println!("Addr: {addr:?}");

    unsafe {
        stack_frame
            .as_mut()
            .update(|x| x.instruction_pointer = addr);
    }
}

// pub struct WithoutInterruptGuard<T> {
//     enabled: bool,
//     inner: T
// }

// impl<T> WithoutInterruptGuard<T> {
//     pub fn new(inner: T) -> Self {
//         let enabled = x86_64::instructions::interrupts::are_enabled();
//         if enabled  {
//             x86_64::instructions::interrupts::disable();
//         }
//         Self {
//             inner,enabled
//         }
//     }
// }

// impl<T> Deref for WithoutInterruptGuard<T> {
//     type Target = T;

//     fn deref(&self) -> &Self::Target {
//         &self.inner
//     }
// }

// impl<T> DerefMut for WithoutInterruptGuard<T> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.inner
//     }
// }

// impl<T> Drop for WithoutInterruptGuard<T> {
//     fn drop(&mut self) {
//         if self.enabled {
//             x86_64::instructions::interrupts::enable();
//         }
//     }
// }

#[test_case]
fn test_breakpoint_exception() {
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
}
