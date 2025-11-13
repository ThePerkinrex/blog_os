use blog_os_device_api::bus::{AssociatedBusData, BusDeviceIdOpaque, cglue_busdeviceid::*};

use api_utils::cglue;

use crate::BUS_NAME;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct PciId {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl core::fmt::Display for PciId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:0>2x}:{:0>2x}:{:0>2x}",
            self.bus, self.device, self.function
        )
    }
}

impl AssociatedBusData<BusDeviceIdOpaque> for PciId {
    fn bus(&self) -> &'static str {
        BUS_NAME
    }

    fn data(&self) -> &BusDeviceIdOpaque {
        let ptr = core::ptr::from_ref(self);
        let ptr: *const BusDeviceIdOpaque = unsafe { core::mem::transmute(ptr) };
        unsafe { ptr.as_ref() }.unwrap() // Ref to myself
    }
}

impl PciId {
    /// # Safety
    /// `id` must be a ref to a PciId
    pub unsafe fn from_opaque(id: &BusDeviceIdOpaque) -> &Self {
        let ptr = core::ptr::from_ref(id);
        let ptr: *const Self = unsafe { core::mem::transmute(ptr) };
        unsafe { ptr.as_ref() }.unwrap() // Ref to myself
    }
}

cglue::cglue_impl_group!(PciId, BusDeviceId);
