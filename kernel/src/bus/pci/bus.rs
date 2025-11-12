use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use blog_os_device::api::{
    bus::{Bus, cglue_busdevicedriver::*, cglue_busdeviceid::*},
    cglue::{
        self,
        iter::{AsCIterator, CIterator},
        trait_obj,
    },
};

use crate::bus::pci::{
    BUS_NAME,
    config::{PciCommonHeader, PciHeaderPCIPCIBridge},
    id::PciId,
};

pub struct PciBus {
    devices: BTreeMap<PciId, (PciCommonHeader, Option<BusDeviceDriverArcBox<'static>>)>,
    drivers: Vec<BusDeviceDriverArcBox<'static>>,
}

impl PciBus {
    pub fn new() -> Self {
        let mut s = Self {
            devices: Default::default(),
            drivers: Default::default(),
        };
        s.full_scan();
        s
    }

    fn full_scan(&mut self) {
        let Some(hdr) = self.function_scan(0, 0, 0) else {
            return;
        };
        if hdr.header_type & 0x80 == 0 {
            // Single PCI host controller
            self.bus_scan(0);
        } else {
            for function in 0..8 {
                let Some(_) = self.function_scan(0, 0, function) else {
                    break;
                };
                self.bus_scan(function);
            }
        }
    }

    fn bus_scan(&mut self, bus: u8) {
        for device in 0..32 {
            self.device_scan(bus, device);
        }
    }

    fn device_scan(&mut self, bus: u8, device: u8) {
        let Some(hdr) = self.function_scan(bus, device, 0) else {
            return;
        };
        if hdr.header_type & 0x80 == 0 {
            // It's a multi-function device, so check remaining functions
            for function in 1..8 {
                self.function_scan(bus, device, function);
            }
        }
    }

    fn function_scan(&mut self, bus: u8, device: u8, function: u8) -> Option<PciCommonHeader> {
        let header = PciCommonHeader::read(bus, device, function)?;
        let id = PciId {
            bus,
            device,
            function,
        };
        self.devices.insert(id, (header, None)); // TODO check for drivers

        if header.class.class() == 0x6
            && header.class.subclass() == 0x4
            && header.header_type == 0x2
        {
            let header = PciHeaderPCIPCIBridge::read(header, bus, device, function);
            self.bus_scan(header.reg6.secondary_bus());
        }

        Some(header)
    }
}

impl Default for PciBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus for PciBus {
    fn name(&self) -> &'static str {
        BUS_NAME
    }

    fn register_driver(&mut self, driver: BusDeviceDriverBox<'static>) {
        todo!()
    }

    fn connected_devices(
        &self,
    ) -> CIterator<'_, (BusDeviceIdRef<'_>, Option<BusDeviceDriverRef<'_>>)> {
        self.devices
            .iter()
            .map(|(id, (_, b))| {
                (
                    trait_obj!(id as BusDeviceId),
                    b.as_ref()
                        .map(|driver| trait_obj!(driver as BusDeviceDriver)),
                )
            })
            .as_citer()
    }
}
