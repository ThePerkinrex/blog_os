use gimli::SectionId;
use object::read::elf::SectionHeader;

use crate::{elf::SystemElf, setup::KernelElfFile};

pub type EndianSlice<'a> = gimli::EndianSlice<'a, gimli::LittleEndian>;
pub type Dwarf<'a> = gimli::Dwarf<EndianSlice<'a>>;

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

fn dwarf_loader<'a>(
    elf: &SystemElf<'a>,
) -> impl FnMut(SectionId) -> Result<EndianSlice<'a>, LoadError> {
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

pub fn load_dwarf<'a>(elf: &SystemElf<'a>) -> Result<Dwarf<'a>, LoadError> {
    Dwarf::load(dwarf_loader(elf))
}
