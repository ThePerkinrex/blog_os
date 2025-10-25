use gimli::{BaseAddresses, EhFrame, EhFrameHdr, EndianSlice, LittleEndian, ParsedEhFrameHdr};
use object::
    read::elf::SectionHeader
;

use crate::{setup::KernelElfFile, println};

#[derive(Debug)]
pub struct EhInfo {
    /// A set of base addresses used for relative addressing.
    pub(super) base_addrs: BaseAddresses,

    /// The parsed `.eh_frame_hdr` section.
    pub(super) hdr: ParsedEhFrameHdr<EndianSlice<'static, LittleEndian>>,

    /// The parsed `.eh_frame` containing the call frame information.
    pub(super) eh_frame: EhFrame<EndianSlice<'static, LittleEndian>>,
}

impl EhInfo {
    pub fn from_elf(elf: &KernelElfFile) -> Option<Self> {
        let eh_frame_hdr_sect = elf
            .elf_section_table()
            .section_by_name(object::LittleEndian, b".eh_frame_hdr")?;
        let eh_frame_sect = elf
            .elf_section_table()
            .section_by_name(object::LittleEndian, b".eh_frame")?;
        println!("eh_frame_hdr: {eh_frame_hdr_sect:?}");
        println!("eh_frame: {eh_frame_sect:?}");

        let eh_frame_hdr = eh_frame_hdr_sect
            .1
            .data(object::LittleEndian, elf.data())
            .expect("eh_frame_hdr");
        let eh_frame = eh_frame_sect
            .1
            .data(object::LittleEndian, elf.data())
            .expect("eh_frame");
        println!("eh_frame_hdr: {eh_frame_hdr:?}");
        println!("eh_frame: {eh_frame:?}");

        let base_addrs = BaseAddresses::default().set_eh_frame_hdr(eh_frame_hdr.as_ptr() as u64).set_eh_frame(eh_frame.as_ptr() as u64);
		let hdr = EhFrameHdr::new(eh_frame_hdr, LittleEndian).parse(&base_addrs, 8).expect("Correct hdr");
		let eh_frame = EhFrame::new(eh_frame, LittleEndian);

        Some(Self { base_addrs, hdr, eh_frame })
    }
}
