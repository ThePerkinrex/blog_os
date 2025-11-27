#![no_std]

use kdriver_api::{KernelInterface, cglue_kernelinterface::KernelInterfaceBox};

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

pub fn print(s: &str) {
    interface().print(s);
}

// Required panic handler
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    interface().abort();
    loop {
        core::hint::spin_loop();
    }
}
