use core::ffi::CStr;

pub trait BusDriver {
    fn bus_name(&self) -> &'static str;
    fn notice_device(&mut self, name: &str);
}

#[repr(C)]
pub struct Bus {
    _data: (),
    _marker:
        core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[repr(C)]
pub struct BusOps {
    name: *const CStr,
    data: *mut Bus,

}