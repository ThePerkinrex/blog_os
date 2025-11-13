use api_utils::cglue;
use blog_os_device_api::bus::{
    AssociatedBusData, BusDeviceMetadataOpaque, cglue_busdevicemetadata::*,
};
use pci_ids::{Device, FromId, Vendor};

use crate::{BUS_NAME, class::PciClass, config::PciCommonHeader};

#[derive(Debug)]
pub struct PciMetadata {
    pub vendor: u16,
    pub device: u16,
    pub subvendor: Option<u16>,
    pub subdevice: Option<u16>,
    pub class: PciClass,
}

impl core::fmt::Display for PciMetadata {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} / {} / {} [{}: {}]: {self:x?}",
            self.class.class_name(),
            self.class.subclass_name(),
            self.class.prog_if_name(),
            self.vendor_name(),
            self.device_name()
        )
    }
}

impl PciMetadata {
    pub fn vendor_name(&self) -> &'static str {
        Vendor::from_id(self.vendor)
            .map(Vendor::name)
            .unwrap_or("Error")
    }
    pub fn device_name(&self) -> &'static str {
        Device::from_vid_pid(self.vendor, self.device)
            .map(Device::name)
            .unwrap_or("Error")
    }
}

impl AssociatedBusData<BusDeviceMetadataOpaque> for PciMetadata {
    fn bus(&self) -> &'static str {
        BUS_NAME
    }

    fn data(&self) -> &BusDeviceMetadataOpaque {
        let ptr = core::ptr::from_ref(self);
        let ptr: *const BusDeviceMetadataOpaque = unsafe { core::mem::transmute(ptr) };
        unsafe { ptr.as_ref() }.unwrap() // Ref to myself
    }
}

impl PciMetadata {
    /// # Safety
    /// `id` must be a ref to a PciId
    pub unsafe fn from_opaque(id: &BusDeviceMetadataOpaque) -> &Self {
        let ptr = core::ptr::from_ref(id);
        let ptr: *const Self = unsafe { core::mem::transmute(ptr) };
        unsafe { ptr.as_ref() }.unwrap() // Ref to myself
    }

    pub const fn from_common_header(hdr: &PciCommonHeader) -> Self {
        Self {
            vendor: hdr.vendor,
            device: hdr.device,
            subvendor: None,
            subdevice: None,
            class: hdr.class,
        }
    }
}

cglue::cglue_impl_group!(PciMetadata, BusDeviceMetadata);
