#![no_std]

#[macro_use]
extern crate kdriver_std;

use core::ffi::CStr;


#[unsafe(no_mangle)]
pub static NAME: &CStr = c"aaaa";
#[unsafe(no_mangle)]
pub static VERSION: &CStr = c"bbbb";

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    println!("Hello from driver");
}

#[unsafe(no_mangle)]
pub extern "C" fn stop() {
    println!("Goodbye from driver");
}
