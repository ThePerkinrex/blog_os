#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use blog_std::println;

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    blog_std::nop(33);
    let x = 22;
    println!("Hello World! {x}");
    println!("Testing alloc");

    let b = Box::new(7);

    let ptr = (b.as_ref() as *const _) as usize;
    blog_std::nop(ptr as u64);

    println!("Alloc box: {b}");

    blog_std::exit(0);
}
