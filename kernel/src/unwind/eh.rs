use gimli::{BaseAddresses, EhFrame, EhFrameHdr, EndianSlice, LittleEndian, ParsedEhFrameHdr};
use log::debug;
use object::read::elf::SectionHeader;

use crate::elf::SystemElf;

#[derive(Debug)]
pub struct EhInfo<'a> {
    /// A set of base addresses used for relative addressing.
    pub(super) base_addrs: BaseAddresses,

    /// The parsed `.eh_frame_hdr` section.
    pub(super) hdr: ParsedEhFrameHdr<EndianSlice<'a, LittleEndian>>,

    /// The parsed `.eh_frame` containing the call frame information.
    pub(super) eh_frame: EhFrame<EndianSlice<'a, LittleEndian>>,
}

impl<'a> EhInfo<'a> {
    pub fn from_elf(elf: &SystemElf<'a>, kernel_image_offset: u64) -> Option<Self> {
        let eh_frame_hdr_sect = elf
            .elf_section_table()
            .section_by_name(object::LittleEndian, b".eh_frame_hdr")?;
        let eh_frame_sect = elf
            .elf_section_table()
            .section_by_name(object::LittleEndian, b".eh_frame")?;
        debug!("eh_frame_hdr: {eh_frame_hdr_sect:?}");
        debug!("eh_frame: {eh_frame_sect:?}");

        let eh_frame_hdr = eh_frame_hdr_sect
            .1
            .data(object::LittleEndian, elf.data())
            .expect("eh_frame_hdr");
        let eh_frame = eh_frame_sect
            .1
            .data(object::LittleEndian, elf.data())
            .expect("eh_frame");
        // println!("eh_frame_hdr: {eh_frame_hdr:?}");
        // println!("eh_frame: {eh_frame:?}");

        // let base_addrs = BaseAddresses::default()
        //     .set_eh_frame_hdr(eh_frame_hdr.as_ptr() as u64)
        //     .set_eh_frame(eh_frame.as_ptr() as u64);

        // println!("Base addresses: {base_addrs:?}");

        let eh_frame_hdr_va = eh_frame_hdr_sect.1.sh_addr.get(object::LittleEndian);
        let eh_frame_va = eh_frame_sect.1.sh_addr.get(object::LittleEndian);

        let base_addrs = BaseAddresses::default()
            // add the runtime kernel image offset (where the ELF is actually loaded)
            .set_eh_frame_hdr(eh_frame_hdr_va + kernel_image_offset)
            .set_eh_frame(eh_frame_va + kernel_image_offset)
            // optionally set a text base too (helps some lookups)
            .set_text(kernel_image_offset);

        debug!("Base addresses: {base_addrs:?}");

        let hdr = EhFrameHdr::new(eh_frame_hdr, LittleEndian)
            .parse(&base_addrs, 8)
            .expect("Correct hdr");
        let eh_frame = EhFrame::new(eh_frame, LittleEndian);

        // let table = hdr.table().unwrap();
        // let mut i = table.iter(&base_addrs);
        // while let Some(Ok((a,b))) = i.next().transpose() {
        //     println!("Lookup ({:x}): {:x} -> {:x}", 0x80000bb989u64, a.pointer(), b.pointer());
        // }
        // panic!();

        // let lookup = table.lookup(0x80000bb989u64, &base_addrs);
        // panic!("{lookup:?}");

        Some(Self {
            base_addrs,
            hdr,
            eh_frame,
        })
    }
}
