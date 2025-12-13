use super::*;
use alloc::collections::btree_map::BTreeMap;
use spin::Lazy;
use x86_64::VirtAddr;

#[derive(Debug)]
pub enum Interrupt {
    // x86-64 Exceptions (first 32 vectors, 0-31)
    Breakpoint,             // Vector 3: #BP
    PageFault,              // Vector 14: #PF
    GeneralProtectionFault, // Vector 13: #GP
    DoubleFault,            // Vector 8: #DF

    // Hardware/Programmable Interrupt Controller (PIC) IRQs (typically vectors 32-255)
    Timer,      // Often mapped to IRQ 0, or Vector 32 (0x20) after PIC remapping
    Keyboard,   // Often mapped to IRQ 1, or Vector 33 (0x21) after PIC remapping
    SystemCall, // Typically for software interrupts like int 0x80
    InterruptTail,
}

pub static IH: Lazy<BTreeMap<VirtAddr, Interrupt>> = Lazy::new(|| {
    let mut map = BTreeMap::new();

    // Exceptions
    map.insert(
        VirtAddr::from_ptr(breakpoint_handler as *const ()),
        Interrupt::Breakpoint,
    );
    map.insert(
        VirtAddr::from_ptr(page_fault_handler as *const ()),
        Interrupt::PageFault,
    );
    map.insert(
        VirtAddr::from_ptr(general_protection_fault_handler as *const ()),
        Interrupt::GeneralProtectionFault,
    );
    map.insert(
        VirtAddr::from_ptr(double_fault_handler as *const ()),
        Interrupt::DoubleFault,
    );

    // Hardware and Software Interrupts
    map.insert(
        VirtAddr::from_ptr(timer_interrupt_handler as *const ()),
        Interrupt::Timer,
    );
    map.insert(
        VirtAddr::from_ptr(keyboard_interrupt_handler as *const ()),
        Interrupt::Keyboard,
    );
    map.insert(
        VirtAddr::from_ptr(naked_int_80_handler as *const ()),
        Interrupt::SystemCall,
    );
    map.insert(
        VirtAddr::from_ptr(stub::interrupt_tail as *const ()),
        Interrupt::InterruptTail,
    );

    map
});
