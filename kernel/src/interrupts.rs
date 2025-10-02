use core::ops::{Deref, DerefMut};

use pic8259::ChainedPics;
use spin::Lazy;
use x86_64::{instructions::port::Port, structures::{idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode}, port::PortWrite}};

use crate::{gdt, println, print};



pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

pub fn init_pics() {
    // init_timer(1);
    unsafe {PICS.lock().initialize();}

    // The Master PIC's Interrupt Mask Register (IMR) is at port 0x21.
    let mut master_mask_port = Port::<u8>::new(0x21);
    
    unsafe {
        // Read the current mask
        let current_mask = master_mask_port.read();
        
        // Clear the lowest bit (IRQ 0/Timer)
        // 0b11111110 (0xFE) ensures only IRQ 0 is unmasked, keeping others as they were.
        master_mask_port.write(current_mask & 0b11111110);
    }

    x86_64::instructions::interrupts::enable();
}
// Function to initialize the PIT to generate interrupts at a specific frequency
pub fn init_timer(frequency_hz: u32) {
    // The PIT base frequency is 1,193,182 Hz
    const PIT_BASE_FREQ: u32 = 1193182;
    
    // Calculate divisor
    let divisor: u16 = (PIT_BASE_FREQ / frequency_hz) as u16;

    // PIT Ports
    const PIT_CMD_PORT: u16 = 0x43;
    const PIT_DATA_PORT: u16 = 0x40; // Channel 0 (Timer)

    // Command: Channel 0, LOBYTE/HIBYTE, Mode 3 (Square Wave Generator)
    const COMMAND: u8 = 0b00110110; 

    // Create mutable port instances
    let mut cmd_port = Port::<u8>::new(PIT_CMD_PORT);
    let mut data_port = Port::<u8>::new(PIT_DATA_PORT);

    unsafe {
        // 1. Send the command byte
        cmd_port.write(COMMAND);
        
        // 2. Send divisor (Low byte then High byte)
        data_port.write((divisor & 0xFF) as u8);
        data_port.write((divisor >> 8) as u8);
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
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
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX); // new
    }
    idt[InterruptIndex::Timer.as_u8()]
            .set_handler_fn(timer_interrupt_handler);
    idt
});

pub fn init_idt() {
    IDT.load();
}

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
    panic!("EXCEPTION: PAGE FAULT ({error_code:?})\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    println!(".");
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
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
