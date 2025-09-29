#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

mod io;

pub fn setup(boot_info: &'static mut bootloader_api::BootInfo) {
    if let Some(fb) = boot_info.framebuffer.as_mut() {
        io::framebuffer::init(fb);
    }
    io::serial::init();
}

pub fn kernel_entrypoint(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    setup(boot_info);

    #[cfg(not(test))]
    kernel_main();
    #[cfg(test)]
    kernel_test();

    #[allow(clippy::empty_loop)]
    loop {}
}

pub fn kernel_main() {
    println!("HELLO");
}

#[cfg(test)]
pub fn kernel_test() {
    println!("TESTING");
    test_main();
}

bootloader_api::entry_point!(kernel_entrypoint);


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


#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    io::qemu::exit_qemu(QemuExitCode::Success);
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 12);
}

use core::panic::PanicInfo;

use qemu_common::QemuExitCode;

/// This function is called on panic.
#[panic_handler]
#[cfg(not(test))]
fn panic(info: &PanicInfo) -> ! {
    use core::fmt::Write;

    if writeln!(io::writer(), "{info}").is_err() {
        io::qemu::exit_qemu(QemuExitCode::PanicWriterFailed);
    }
    loop {}
}

#[panic_handler]
#[cfg(test)]
fn panic(info: &PanicInfo) -> ! {
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
