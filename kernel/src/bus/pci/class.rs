use bitfield_struct::bitfield;

#[bitfield(u32)]
pub struct PciClass {
    pub class: u8,
    pub subclass: u8,
    pub interface: u8,
    pub revision: u8,
}
