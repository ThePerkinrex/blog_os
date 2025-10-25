use gimli::{BaseAddresses, EhFrame, EhHdrTable, EndianSlice, LittleEndian, ParsedEhFrameHdr};
use object::{read::elf::{ElfSection, SectionHeader}, Object};

use crate::{println, KernelElfFile};

pub struct EhInfo {
    /// A set of base addresses used for relative addressing.
    base_addrs: BaseAddresses,

    /// The parsed `.eh_frame_hdr` section.
    hdr: &'static ParsedEhFrameHdr<EndianSlice<'static, LittleEndian>>,

    /// The lookup table within the parsed `.eh_frame_hdr` section.
    hdr_table: EhHdrTable<'static, EndianSlice<'static, LittleEndian>>,

    /// The parsed `.eh_frame` containing the call frame information.
    eh_frame: EhFrame<EndianSlice<'static, LittleEndian>>,
}

impl EhInfo {
	pub fn from_elf(elf: &KernelElfFile) -> Option<Self> {
		let eh_frame_hdr_sect = elf.elf_section_table().section_by_name(object::LittleEndian, b".eh_frame_hdr")?;
		let eh_frame_sect = elf.elf_section_table().section_by_name(object::LittleEndian, b".eh_frame")?;
		println!("eh_frame_hdr: {eh_frame_hdr_sect:?}");
		println!("eh_frame: {eh_frame_sect:?}");

		let eh_frame_hdr = eh_frame_hdr_sect.1.data(object::LittleEndian, elf.data()).expect("eh_frame_hdr");
		let eh_frame = eh_frame_sect.1.data(object::LittleEndian, elf.data()).expect("eh_frame");
		println!("eh_frame_hdr: {eh_frame_hdr:?}");
		println!("eh_frame: {eh_frame:?}");

		
		todo!()
	}
}