#![no_std]

extern crate kdriver_std;

use core::ffi::CStr;

use kdriver_std::print;

#[unsafe(no_mangle)]
pub static NAME: &CStr = c"aaaa";
#[unsafe(no_mangle)]
pub static VERSION: &CStr = c"bbbb";

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    print("Hello from driver");
}

#[unsafe(no_mangle)]
pub extern "C" fn stop() {
    print("Goodbye from driver");
}
