#![no_std]
#![no_main]

use core::fmt::Write;


pub fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    let fb = boot_info.framebuffer.as_mut().unwrap();
    let info = fb.info();
    let buffer = fb.buffer_mut();
    let mut fb_writer = FrameBufferWriter::new(buffer, info);
    fb_writer.clear();
    writeln!(fb_writer, "Hello world!").unwrap();
    loop {}
}

bootloader_api::entry_point!(kernel_main);


use core::panic::PanicInfo;

use bootloader_x86_64_common::framebuffer::FrameBufferWriter;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}