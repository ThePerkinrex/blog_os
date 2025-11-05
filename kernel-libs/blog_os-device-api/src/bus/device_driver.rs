use core::ffi::CStr;

use alloc::boxed::Box;

#[repr(C)]
pub struct BusDeviceDriver {
    _data: (),
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[repr(C)]
pub struct BusDeviceDriverOps {
    pub name: *const CStr,
    pub data: *mut BusDeviceDriver,
    free: extern "C" fn(*mut BusDeviceDriver),
}

impl Drop for BusDeviceDriverOps {
    fn drop(&mut self) {
        (self.free)(self.data)
    }
}

impl<B: RustBusDeviceDriver + 'static> From<B> for BusDeviceDriverOps {
    fn from(value: B) -> Self {
        let data = Box::leak::<'static>(Box::new(value));
        let p = core::ptr::from_mut(data);
        extern "C" fn free<B: RustBusDeviceDriver + 'static>(bus: *mut BusDeviceDriver) {
            let bus = unsafe { Box::from_raw(bus as *mut B) };
            bus.free();
        }

        Self {
            name: B::NAME,
            data: p as *mut BusDeviceDriver,
            free: free::<B>,
        }
    }
}

pub trait RustBusDeviceDriver {
    const NAME: &'static CStr;

    fn free(self);
}