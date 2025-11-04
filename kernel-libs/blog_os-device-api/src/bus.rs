use core::ffi::CStr;

use alloc::boxed::Box;

pub trait BusDriver {
    fn bus_name(&self) -> &'static str;
    fn notice_device(&mut self, name: &str);
}

#[repr(C)]
pub struct Bus {
    _data: (),
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[repr(C)]
pub struct BusOps {
    pub name: *const CStr,
    pub data: *mut Bus,
    free_bus: extern "C" fn(*mut Bus),
}

impl Drop for BusOps {
    fn drop(&mut self) {
        (self.free_bus)(self.data)
    }
}

impl<B: RustBus + 'static> From<B> for BusOps {
    fn from(value: B) -> Self {
        let data = Box::leak::<'static>(Box::new(value));
        let p = core::ptr::from_mut(data);
        extern "C" fn free_bus<B: RustBus + 'static>(bus: *mut Bus) {
            let bus = unsafe { Box::from_raw(bus as *mut B) };
            bus.free();
        }

        Self {
            name: B::NAME,
            data: p as *mut Bus,
            free_bus: free_bus::<B>,
        }
    }
}

pub trait RustBus {
    const NAME: &'static CStr;

    fn free(self);
}

