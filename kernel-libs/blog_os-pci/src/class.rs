use bitfield_struct::bitfield;
use pci_ids::{Class, FromId, Subclass};

#[bitfield(u32)]
pub struct PciClass {
    pub revision: u8,
    pub interface: u8,
    pub subclass: u8,
    pub class: u8,
}

impl PciClass {
    pub fn class_name(&self) -> &'static str {
        Class::from_id(self.class())
            .map(|x| x.name())
            .unwrap_or("Error")
    }
    pub fn subclass_name(&self) -> &'static str {
        Subclass::from_cid_sid(self.class(), self.subclass())
            .map(|x| x.name())
            .unwrap_or("Error")
    }
    pub fn prog_if_name(&self) -> &'static str {
        Subclass::from_cid_sid(self.class(), self.subclass())
            .and_then(|x| x.prog_ifs().find(|x| x.id() == self.interface()))
            .map(|x| x.name())
            .unwrap_or("Error")
    }
}
