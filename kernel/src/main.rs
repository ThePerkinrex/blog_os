#![no_std]
#![no_main]

#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use blog_os::{println, print};


#[cfg(not(test))]
pub fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    let fb = boot_info.framebuffer.as_mut().unwrap();
    blog_os::framebuffer::init(fb);
    println!("Not test");



    loop {}
}

#[cfg(test)]
pub fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    let fb = boot_info.framebuffer.as_mut().unwrap();
    blog_os::framebuffer::init(fb);
    test_main();

    loop {}
}


bootloader_api::entry_point!(kernel_main);

