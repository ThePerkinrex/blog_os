#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    loop{}
}

// Required panic handler
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}