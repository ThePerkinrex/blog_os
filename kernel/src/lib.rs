#![no_std]
#![cfg_attr(test, no_main)]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "kernel_test"]

pub mod gdt;
pub mod interrupts;
pub mod io;

pub fn setup(boot_info: &'static mut bootloader_api::BootInfo) {
    gdt::init();
    interrupts::init_idt();
    if let Some(fb) = boot_info.framebuffer.as_mut() {
        io::framebuffer::init(fb);
    }
    io::serial::init();
}

pub fn kernel_main() {
    println!("HELLO");
    x86_64::instructions::interrupts::int3(); // new
    println!("DID NOT CRASH!");
}

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        print!("{}...\t", core::any::type_name::<T>());
        self();
        println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    io::qemu::exit_qemu(QemuExitCode::Success);
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}

use core::panic::PanicInfo;

use qemu_common::QemuExitCode;

pub fn panic_handler(info: &PanicInfo) -> ! {
    use core::fmt::Write;

    if writeln!(io::writer(), "{info}").is_err() {
        io::qemu::exit_qemu(QemuExitCode::PanicWriterFailed);
    }
    loop {}
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    use core::fmt::Write;

    use crate::io::qemu::exit_qemu;

    if writeln!(io::writer(), "[failed]\n").is_err() {
        io::qemu::exit_qemu(QemuExitCode::PanicWriterFailed);
    }

    if writeln!(io::writer(), "Error: {info}\n").is_err() {
        io::qemu::exit_qemu(QemuExitCode::PanicWriterFailed);
    }
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

#[cfg(test)]
pub fn kernel_test_entrypoint(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    setup(boot_info);

    kernel_test();

    #[allow(clippy::empty_loop)]
    loop {}
}

#[cfg(test)]
bootloader_api::entry_point!(kernel_test_entrypoint);

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}
