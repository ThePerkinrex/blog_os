#![no_std]
#![cfg_attr(test, no_main)]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::panic::PanicInfo;

use log::info;
use qemu_common::QemuExitCode;

use crate::{process::ProcessInfo, setup::KERNEL_INFO};

pub mod allocator;
pub mod config;
pub mod dwarf;

#[allow(clippy::future_not_send)]
pub mod elf;
pub mod gdt;
pub mod interrupts;
pub mod io;
pub mod memory;
pub mod multitask;
pub mod priviledge;
pub mod process;
pub mod rand;
pub mod setup;
pub mod stack;
pub mod unwind;
pub mod util;

pub fn kernel_main() -> ! {
    // let addresses = [
    //     // the identity-mapped vga buffer page
    //     0xb8000,
    //     // some code page
    //     0x201008,
    //     // some stack page
    //     0x0100_0020_1a10,
    // ];

    // let setup_info = KERNEL_INFO.get().unwrap();
    // let setup_info = setup_info.alloc_kinf.lock();
    // for &address in &addresses {
    //     let virt = VirtAddr::new(address);
    //     // new: use the `mapper.translate_addr` method
    //     let phys = setup_info.page_table.translate_addr(virt);
    //     println!("{:?} -> {:?}", virt, phys);
    // }
    // drop(setup_info);

    info!("Adding new task to list");
    multitask::create_task(other_task, "other_task");

    info!("DID NOT CRASH!");
    info!("Switching");
    multitask::task_switch_safe();
    info!("Returned");
    info!("Going back");
    multitask::task_switch_safe();

    multitask::create_task(switch_loop, "switch loop");
    multitask::create_task(second_process, "second_process");

    // println!("JUmping to user mode");
    // test_jmp_to_usermode();
    let prog = elf::load_example_elf();
    let proc = ProcessInfo::new(prog);
    proc.start();
    hlt_loop()
}

pub extern "C" fn other_task() {
    info!("STarted other task");
    info!("Switching back");
    multitask::task_switch_safe();
    info!("Should not be here");
}

pub extern "C" fn second_process() {
    info!("Starting second process");
    let prog = elf::load_example_elf();
    let proc = ProcessInfo::new(prog);
    proc.start();
}

// pub extern "C" fn test_return() -> ! {
//     info!("REturned here");
//     hlt_loop();
// }

pub extern "C" fn switch_loop() {
    x86_64::instructions::interrupts::enable();
    loop {
        info!("SWITCH LOOP - Waiting");
        x86_64::instructions::interrupts::enable_and_hlt();
        info!("SWITCH LOOP - Switching");
        multitask::task_switch_safe();
    }
}

#[cfg(test)]
pub fn kernel_test() -> ! {
    test_main();
    hlt_loop()
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        _print!("{}...\t", core::any::type_name::<T>());
        self();
        _println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    info!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    io::qemu::exit_qemu(QemuExitCode::Success);
}

pub fn panic_handler(info: &PanicInfo) -> ! {
    use core::fmt::Write;

    if io::writer(|mut w| writeln!(w, "{info}")).is_err() {
        io::qemu::exit_qemu(QemuExitCode::PanicWriterFailed);
    }
    hlt_loop()
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    use core::fmt::Write;

    use crate::io::qemu::exit_qemu;

    if io::writer(|mut w| writeln!(w, "[failed]\n")).is_err() {
        io::qemu::exit_qemu(QemuExitCode::PanicWriterFailed);
    }

    if io::writer(|mut w| writeln!(w, "Error: {info}\n")).is_err() {
        io::qemu::exit_qemu(QemuExitCode::PanicWriterFailed);
    }
    exit_qemu(QemuExitCode::Failed);
    hlt_loop()
}

#[cfg(test)]
pub fn kernel_test_entrypoint(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    setup::setup(boot_info);

    kernel_test()
}

#[cfg(test)]
bootloader_api::entry_point!(kernel_test_entrypoint, config = &config::BOOTLOADER_CONFIG);

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}
