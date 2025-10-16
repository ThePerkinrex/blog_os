#![no_std]
#![no_main]

use core::panic::PanicInfo;

fn exit() -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
        blog_std::exit(0);
    exit()
}

// Required panic handler
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit()
}
