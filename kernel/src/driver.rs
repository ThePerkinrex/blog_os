use kdriver_api::{CLayout, KernelInterface};

pub struct Interface {}

impl KernelInterface for Interface {
    fn abort(&self) {
        todo!()
    }

    fn print(&self, str: &str) {
        todo!()
    }
    unsafe fn alloc(&self, layout: CLayout) -> *mut u8 {
        todo!()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: CLayout) {
        todo!()
    }
}
