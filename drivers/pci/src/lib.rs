#![no_std]

#[macro_use]
extern crate kdriver_std;

extern crate alloc;

mod bus;

use core::ffi::CStr;

use bus::PciBus;
use kdriver_std::{
    api::{KernelInterface, cglue, device::bus::cglue_bus::*},
    interface,
};

#[unsafe(no_mangle)]
pub static NAME: &CStr = c"pci";
#[unsafe(no_mangle)]
pub static VERSION: &CStr = c"0.1.0";

#[unsafe(no_mangle)]
pub extern "C" fn start() {
    println!("starting pci");

    let bus = PciBus::new();

    interface().register_bus(cglue::trait_obj!(bus as Bus));
}

#[unsafe(no_mangle)]
pub extern "C" fn stop() {
    println!("stopping pci");
}
