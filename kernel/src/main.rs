#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "kernel_test"]

use core::panic::PanicInfo;

#[cfg(not(test))]
use blog_os::kernel_main;
use blog_os::{panic_handler, setup};

pub fn kernel_entrypoint(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    setup(boot_info);

    #[cfg(test)]
    kernel_test();

    #[cfg(not(test))]
    kernel_main();

    #[allow(clippy::empty_loop)]
    loop {}
}

#[test_case]
fn trivial_assertion_bin() {
    assert_eq!(1, 1);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    panic_handler(info)
}

bootloader_api::entry_point!(kernel_entrypoint);
