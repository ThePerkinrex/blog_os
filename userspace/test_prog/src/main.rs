#![no_std]
#![no_main]

use blog_std::println;

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    blog_std::nop(33);
    println!("Hello World!");
    blog_std::exit(0);
}
