#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use blog_std::println;

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    blog_std::nop(33);
    println!("Hello World!");
    println!("Testing alloc");

    let b = Box::new(7);

    println!("Alloc box: {b}");

    blog_std::exit(0);
}
