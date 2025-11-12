use crate::bus::pci::class::PciClass;

pub struct PciMetadata {
    pub vendor: u16,
    pub device: u16,
    pub subvendor: Option<u16>,
    pub subdevice: Option<u16>,
    pub class: PciClass,
}
