#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

use blog_os_kernel::{panic_handler, setup};

#[cfg(test)]
pub fn kernel_entrypoint(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    use blog_os_kernel::hlt_loop;

    setup::setup(boot_info);

    test_main();
    hlt_loop();
}

#[cfg(not(test))]
pub fn kernel_entrypoint(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    use blog_os_kernel::kernel_main;
    setup::setup(boot_info);

    kernel_main()
}

// #[test_case]
// fn trivial_assertion_bin() {
//     assert_eq!(1, 1);
// }

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    panic_handler(info)
}

bootloader_api::entry_point!(
    kernel_entrypoint,
    config = &blog_os_kernel::config::BOOTLOADER_CONFIG
);
