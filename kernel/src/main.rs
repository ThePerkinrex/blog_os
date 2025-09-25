#![no_std]
#![no_main]

mod framebuffer;

pub fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    let fb = boot_info.framebuffer.as_mut().unwrap();
    framebuffer::init(fb);
    println!("Hello");
    println!("World!");
    loop {}
}

bootloader_api::entry_point!(kernel_main);

use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
