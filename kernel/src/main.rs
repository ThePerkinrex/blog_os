#![no_std]
#![no_main]

#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

mod io;


pub fn kernel_entrypoint(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    #[cfg(test)]
    kernel_test(boot_info);
    #[cfg(not(test))]
    kernel_main(boot_info);
    
}

pub fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    let fb = boot_info.framebuffer.as_mut().unwrap();
    framebuffer::init(fb);

    loop {}
}


#[cfg(test)]
pub fn kernel_test(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    let fb = boot_info.framebuffer.as_mut().unwrap();
    framebuffer::init(fb);
    test_main();

    loop {}
}


bootloader_api::entry_point!(kernel_main);


#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}

#[test_case]
fn trivial_assertion() {
    print!("trivial assertion... ");
    assert_eq!(1, 1);
    println!("[ok]");
}

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
