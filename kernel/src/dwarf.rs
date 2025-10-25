use gimli::SectionId;
use object::read::elf::SectionHeader;

use crate::setup::KernelElfFile;

pub type EndianSlice = gimli::EndianSlice<'static, gimli::LittleEndian>;
pub type Dwarf = gimli::Dwarf<EndianSlice>;

#[derive(Debug)]
pub enum LoadError {
	Object(object::read::Error),
	NotFound(SectionId)
}

impl From<object::read::Error> for LoadError {
	fn from(value: object::read::Error) -> Self {
		Self::Object(value)
	}
}

fn dwarf_loader(elf: &KernelElfFile) -> impl FnMut(SectionId) -> Result<EndianSlice, LoadError> {
	|s| {
		let (_, hdr) = elf
            .elf_section_table()
            .section_by_name(object::LittleEndian, s.name().as_bytes()).ok_or(LoadError::NotFound(s))?;

        let sect = hdr
            .data(object::LittleEndian, elf.data())?;

		Ok(EndianSlice::new(sect, gimli::LittleEndian))
	}
}

pub fn load_dwarf(elf: &KernelElfFile) -> Result<Dwarf, LoadError> {
	Dwarf::load(dwarf_loader(elf))
}


