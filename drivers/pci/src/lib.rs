#![no_std]

extern crate kdriver_std;

use core::ffi::CStr;

use kdriver_std::print;

#[unsafe(no_mangle)]
pub static NAME: &CStr = c"pci";
#[unsafe(no_mangle)]
pub static VERSION: &CStr = c"0.1.0";

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    print("starting pci");
}

#[unsafe(no_mangle)]
pub extern "C" fn stop() {
    print("stopping pci");
}