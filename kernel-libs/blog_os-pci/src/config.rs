use bitfield_struct::bitfield;
use spin::Mutex;
use x86_64::instructions::port::{Port, PortWriteOnly};

use crate::class::PciClass;

struct Ports {
    config: PortWriteOnly<u32>,
    data: Port<u32>,
}

static PORTS: Mutex<Ports> = Mutex::new(Ports {
    config: PortWriteOnly::new(0xCF8),
    data: Port::new(0xCFC),
});

#[bitfield(u32)]
pub struct PciConfigAddr {
    pub offset: u8,
    #[bits(3)]
    pub function: u8,
    #[bits(5)]
    pub device: u8,
    pub bus: u8,
    #[bits(7)]
    pub reserved: u8,
    pub enabled: bool,
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
pub struct PciReg0 {
    pub vendor: u16,
    pub device: u16,
}

#[bitfield(u32)]
pub struct PciReg3 {
    pub cache_line_size: u8,
    pub latency_timer: u8,
    pub header_type: u8,
    pub bist: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct PciCommonHeader {
    pub device: u16,
    pub vendor: u16,
    pub class: PciClass,
    pub header_type: u8,
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
pub struct PciReg6PCIPCIBridge {
    pub primary_bus: u8,
    pub secondary_bus: u8,
    pub subordinate_bus: u8,
    pub secondary_latency_timer: u8,
}

pub struct PciHeaderPCIPCIBridge {
    pub common: PciCommonHeader,
    pub reg6: PciReg6PCIPCIBridge,
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
