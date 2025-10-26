use gimli::SectionId;
use object::read::elf::SectionHeader;

use crate::setup::KernelElfFile;

pub type EndianSlice = gimli::EndianSlice<'static, gimli::LittleEndian>;
pub type Dwarf = gimli::Dwarf<EndianSlice>;

#[derive(Debug)]
pub enum LoadError {
    Object(object::read::Error),
    NotFound(SectionId),
}

impl From<object::read::Error> for LoadError {
    fn from(value: object::read::Error) -> Self {
        Self::Object(value)
    }
}

const EMPTY: &[u8] = &[];

fn dwarf_loader(elf: &KernelElfFile) -> impl FnMut(SectionId) -> Result<EndianSlice, LoadError> {
    |s| {
        if let Some(hdr) = elf
            .elf_section_table()
            .section_by_name(object::LittleEndian, s.name().as_bytes())
            .map(|(_, x)| x)
        {
            let sect = hdr.data(object::LittleEndian, elf.data())?;

            Ok(EndianSlice::new(sect, gimli::LittleEndian))
        } else {
            Ok(EndianSlice::new(EMPTY, gimli::LittleEndian))
        }
    }
}

pub fn load_dwarf(elf: &KernelElfFile) -> Result<Dwarf, LoadError> {
    Dwarf::load(dwarf_loader(elf))
}
