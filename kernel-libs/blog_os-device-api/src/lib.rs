#![no_std]

extern crate alloc;

pub mod bus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct DeviceId {
    pub major: u64,
    pub minor: u64,
}

#[macro_export]
macro_rules! opaque_ptr {
    ($name:ident) => {
        #[repr(C)]
        pub struct $name {
            _data: (),
            _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
        }
    };
}
