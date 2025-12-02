#![no_std]

extern crate alloc;

use alloc::string::String;
use kdriver_api::{KernelInterface, cglue_kernelinterface::KernelInterfaceBox};

pub use kdriver_api as api;

unsafe extern "C" {
    unsafe static ID: u64;
    unsafe fn get_interface<'a>(id: u64) -> *const KernelInterfaceBox<'static>;
}

pub fn interface<'a>() -> &'a KernelInterfaceBox<'static> {
    unsafe { get_interface(ID).as_ref() }.unwrap()
}

struct GlobalAllocator;

unsafe impl core::alloc::GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        unsafe { interface().alloc(layout.into()) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        unsafe { interface().dealloc(ptr, layout.into()) }
    }
}

#[global_allocator]
static ALLOC: GlobalAllocator = GlobalAllocator;

pub fn print(s: core::fmt::Arguments) {
    use core::fmt::Write;
    let mut string = String::new();
    string.write_fmt(s).unwrap();
    interface().print(&string);
}

// Required panic handler
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    interface().abort();
    loop {
        core::hint::spin_loop();
    }
}


#[macro_export]
macro_rules! _print {
    ($($arg:tt)*) => ($crate::print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::_print!("\n"));
    ($($arg:tt)*) => ($crate::_print!("{}\n", format_args!($($arg)*)));
}

