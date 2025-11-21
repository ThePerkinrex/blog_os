#![no_std]

use kdriver_api::{KernelInterface, cglue_kernelinterface::KernelInterfaceRef};

unsafe extern "C" {
    pub unsafe static INTERFACE: KernelInterfaceRef<'static>;
}

struct GlobalAllocator;

unsafe impl core::alloc::GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        unsafe { INTERFACE.alloc(layout.into()) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        unsafe { INTERFACE.dealloc(ptr, layout.into()) }
    }
}

#[global_allocator]
static ALLOC: GlobalAllocator = GlobalAllocator;

pub fn print(s: &str) {
    unsafe { &INTERFACE }.print(s);
}

// Required panic handler
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    unsafe { &INTERFACE }.abort();
    loop {
        core::hint::spin_loop();
    }
}
