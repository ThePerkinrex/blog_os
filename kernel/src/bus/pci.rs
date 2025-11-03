use core::{cell::Ref, str::FromStr};

use alloc::{
    borrow::Cow,
    boxed::Box,
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    string::ToString,
    vec::Vec,
};
use bitfield_struct::bitfield;
use blog_os_device::api::bus::BusDriver;
use num_traits::Num;
use spin::Mutex;
use x86_64::instructions::port::{Port, PortWriteOnly};

use crate::bus::{Bus, PatternParseError};

#[bitfield(u32)]
struct PciClass {
    class: u8,
    subclass: u8,
    interface: u8,
    revision: u8,
}

struct PciPattern {
    vendor: Option<u16>,    // None means any
    device: Option<u16>,    // None means any
    subvendor: Option<u16>, // None means any
    subdevice: Option<u16>, // None means any
    class: PciClass,
    classmask: PciClass,
}

fn parse_num<N: Num>(s: &str) -> Result<N, N::FromStrRadixErr> {
    #[allow(clippy::option_if_let_else)]
    if let Some(s) = s.strip_prefix("0x") {
        N::from_str_radix(s, 16)
    } else if let Some(s) = s.strip_prefix("0b") {
        N::from_str_radix(s, 2)
    } else if let Some(s) = s.strip_prefix("0o") {
        N::from_str_radix(s, 8)
    } else {
        N::from_str_radix(s, 10)
    }
}

fn parse_or_any<F: FnOnce(&str) -> Result<T, E>, T, E>(s: &str, f: F) -> Result<Option<T>, E> {
    if s == "*" { Ok(None) } else { f(s).map(Some) }
}

impl FromStr for PciPattern {
    type Err = PatternParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split_whitespace();
        let res = Ok(Self {
            vendor: parse_or_any(parts.next().ok_or(PatternParseError)?, parse_num)
                .map_err(|_| PatternParseError)?,
            device: parse_or_any(parts.next().ok_or(PatternParseError)?, parse_num)
                .map_err(|_| PatternParseError)?,
            subvendor: parse_or_any(parts.next().ok_or(PatternParseError)?, parse_num)
                .map_err(|_| PatternParseError)?,
            subdevice: parse_or_any(parts.next().ok_or(PatternParseError)?, parse_num)
                .map_err(|_| PatternParseError)?,
            class: PciClass::from_bits(
                parse_num(parts.next().ok_or(PatternParseError)?).map_err(|_| PatternParseError)?,
            ),
            classmask: PciClass::from_bits(
                parse_num(parts.next().ok_or(PatternParseError)?).map_err(|_| PatternParseError)?,
            ),
        });
        if parts.next().is_none() {
            res
        } else {
            Err(PatternParseError)
        }
    }
}

impl PciPattern {}

struct PciMetadata {
    vendor: u16,
    device: u16,
    subvendor: Option<u16>,
    subdevice: Option<u16>,
    class: PciClass,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PciId {
    bus: u8,
    device: u8,
    function: u8,
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

pub struct PciBus {
    devices: BTreeMap<PciId, (PciCommonHeader, Option<Box<dyn BusDriver>>)>,
    drivers: Vec<(Box<dyn BusDriver>, PciPattern)>,
}

struct Ports {
    config: PortWriteOnly<u32>,
    data: Port<u32>,
}

static PORTS: Mutex<Ports> = Mutex::new(Ports {
    config: PortWriteOnly::new(0xCF8),
    data: Port::new(0xCFC),
});

#[bitfield(u32)]
struct PciConfigAddr {
    offset: u8,
    #[bits(3)]
    function: u8,
    #[bits(5)]
    device: u8,
    bus: u8,
    #[bits(7)]
    reserved: u8,
    enabled: bool,
}

/// Offset will be truncated to dword (32bit) alignement
fn read_pci_dword(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let addr = PciConfigAddr::new()
        .with_bus(bus)
        .with_device(device)
        .with_function(function)
        .with_enabled(true)
        .with_offset(offset & 0b11111000);

    let mut ports = PORTS.lock();

    unsafe { ports.config.write(addr.into_bits()) };

    unsafe { ports.data.read() }
}

#[bitfield(u32)]
struct PciReg0 {
    device: u16,
    vendor: u16,
}

#[bitfield(u32)]
struct PciReg3 {
    bist: u8,
    header_type: u8,
    latency_timer: u8,
    cache_line_size: u8,
}

#[derive(Debug, Clone, Copy)]
struct PciCommonHeader {
    device: u16,
    vendor: u16,
    class: PciClass,
    header_type: u8,
}

impl PciCommonHeader {
    pub fn read(bus: u8, device: u8, function: u8) -> Option<Self> {
        let reg0 = PciReg0::from_bits(read_pci_dword(bus, device, function, 0x0));
        if reg0.vendor() == 0xFFFF {
            return None;
        }

        let class = PciClass::from_bits(read_pci_dword(bus, device, function, 0x8));
        let reg3 = PciReg3::from_bits(read_pci_dword(bus, device, function, 0xC));
        Some(Self {
            device: reg0.device(),
            vendor: reg0.vendor(),
            class,
            header_type: reg3.header_type(),
        })
    }
}

#[bitfield(u32)]
struct PciReg6PCIPCIBridge {
    secondary_latency_timer: u8,
    subordinate_bus: u8,
    secondary_bus: u8,
    primary_bus: u8,
}

struct PciHeaderPCIPCIBridge {
    common: PciCommonHeader,
    reg6: PciReg6PCIPCIBridge,
}

impl PciHeaderPCIPCIBridge {
    pub fn read(common: PciCommonHeader, bus: u8, device: u8, function: u8) -> Self {
        debug_assert_eq!(common.header_type, 0x1);
        Self {
            common,
            reg6: PciReg6PCIPCIBridge::from_bits(read_pci_dword(bus, device, function, 0x18)),
        }
    }
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
        "pci"
    }

    fn devices(&self) -> alloc::boxed::Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        Box::new(self.devices.keys().map(ToString::to_string).map(Into::into))
    }

    fn register_driver(
        &mut self,
        pattern: &str,
        driver: Box<dyn BusDriver>,
    ) -> Result<(), PatternParseError> {
        todo!()
    }
}
