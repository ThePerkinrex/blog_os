use core::{alloc::Layout, ops::DerefMut};

use alloc::{boxed::Box, collections::btree_map::BTreeMap, vec::Vec};
use object::{
    LittleEndian,
    read::elf::{ElfFile64, FileHeader, ProgramHeader},
};
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, Page, PageSize, PageTableFlags, Size4KiB},
};

use crate::{
    setup::KERNEL_INFO, println,
    stack::{self, GeneralStack},
    util::MaybeBoxed,
};

const TEST: &[u8] = include_bytes!("./progs/test_prog");

pub type SystemElf<'a> = ElfFile64<'a, LittleEndian, &'a [u8]>;

fn copy_aligned_box(align: usize, og: &[u8]) -> Box<[u8]> {
    let size = og.len();
    let layout = Layout::from_size_align(size, align).unwrap();
    unsafe {
        let ptr = alloc::alloc::alloc(layout);
        if ptr.is_null() {
            panic!("allocation failed");
        }
        let slice = core::slice::from_raw_parts_mut(ptr, size);
        slice.copy_from_slice(og);
        Box::from_raw(slice)
    }
}

fn realign_if_necessary<'a>(align: usize, og: &'a [u8]) -> MaybeBoxed<'a, [u8]> {
    if (og.as_ptr() as usize).is_multiple_of(align) {
        MaybeBoxed::Borrowed(og)
    } else {
        MaybeBoxed::Boxed(copy_aligned_box(align, og))
    }
}

#[derive(Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PHType {
    Null = 0,
    Load,
    Dynamic,
    Interp,
    Note,
    Phdr = 6,
    Other(u32),
}

impl From<u32> for PHType {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Null,
            1 => Self::Load,
            2 => Self::Dynamic,
            3 => Self::Interp,
            4 => Self::Note,
            6 => Self::Phdr,
            x => Self::Other(x),
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug)]
    pub struct ElfPhSegmentFlags: u32 {
        /// Executable
        const E = 0b00000001;
        /// Writable
        const W = 0b00000010;
        /// READABLE
        const R = 0b00000100;
    }
}

#[derive(Debug)]
pub struct LoadedProgram {
    // TODO mem
    stack: GeneralStack,
    entry: VirtAddr,
}

impl LoadedProgram {
    pub const fn entry(&self) -> VirtAddr {
        self.entry
    }

    pub const fn stack(&self) -> &GeneralStack {
        &self.stack
    }
}

pub fn load_elf(bytes: &[u8]) -> LoadedProgram {
    let align = core::mem::align_of::<object::elf::FileHeader64<object::endian::LittleEndian>>();

    let aligned = realign_if_necessary(align, bytes);

    let elf = SystemElf::parse(&aligned).expect("Correct ELF");
    // TODO check if its executable

    // TODO verify general header

    println!("ELF type: {:x}", elf.elf_header().e_type(LittleEndian));

    // Show prog headers
    let mut loads = Vec::<((u64, u64), (VirtAddr, u64), ElfPhSegmentFlags)>::new();
    println!(
        "                type      flags     offset      vaddr      paddr     filesz      memsz      align"
    );
    for p in elf.elf_program_headers() {
        let p_type = PHType::from(p.p_type(LittleEndian));
        let offset = p.p_offset(LittleEndian);
        let filesz = p.p_filesz(LittleEndian);
        let vaddr = p.p_vaddr(LittleEndian);
        let memsz = p.p_memsz(LittleEndian);
        let flags = p.p_flags(LittleEndian);
        let flags = ElfPhSegmentFlags::from_bits_retain(flags);
        println!(
            "{:>20} {:>10?} {:>10x} {:>10x} {:>10x} {:>10x} {:>10x} {:>10x}",
            alloc::format!("{:?}", p_type),
            flags,
            offset,
            vaddr,
            p.p_paddr(LittleEndian),
            filesz,
            memsz,
            p.p_align(LittleEndian)
        );
        if p_type == PHType::Load {
            loads.push(((offset, filesz), (VirtAddr::new(vaddr), memsz), flags));
        }
    }

    let info = KERNEL_INFO.get().unwrap();
    let mut info = info.alloc_kinf.lock();
    let info = info.deref_mut();
    let mut highest_page: Option<Page> = None;
    let mut mapped_pages = BTreeMap::new();
    for ((offset, filesz), (vaddr, memsz), flags) in loads {
        println!(
            "Loading segment at offset {offset:x} (sz: {filesz:x}) to {vaddr:p} (sz: {memsz:x})"
        );
        let pages = Page::<Size4KiB>::range_inclusive(
            Page::containing_address(vaddr),
            Page::containing_address(vaddr + memsz),
        );
        for p in pages {
            let mut page_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE; // For now with one page table we should be able to write to it
            if !flags.contains(ElfPhSegmentFlags::E) {
                page_flags |= PageTableFlags::NO_EXECUTE
            }
            if flags.contains(ElfPhSegmentFlags::W) {
                page_flags |= PageTableFlags::WRITABLE
            }
            if flags.contains(ElfPhSegmentFlags::R) {
                page_flags |= PageTableFlags::USER_ACCESSIBLE
            }

            if let Some(x) = mapped_pages.get(&p) {
                println!("Skipping {p:?}. Already mapped with flags {x:?}");
                if page_flags != *x {
                    println!(
                        "WARN Unexpected page flags ({page_flags:?}) different from already mapped ({x:?})"
                    )
                }
            } else {
                println!("Mapping {p:?} with flags {page_flags:?}");
                let frame = info.frame_allocator.allocate_frame().expect("A frame");
                unsafe {
                    info.page_table.map_to(
                        p,
                        frame,
                        PageTableFlags::PRESENT
                            | PageTableFlags::WRITABLE
                            | PageTableFlags::USER_ACCESSIBLE,
                        &mut info.frame_allocator,
                    )
                }
                .unwrap()
                .flush();
                mapped_pages.insert(p, page_flags);
            }
        }
        let elf_start = VirtAddr::from_ptr(aligned.as_ptr()) + offset;
        println!("Copying from {elf_start:p} to {vaddr:p} {filesz:x} bytes");
        unsafe {
            core::ptr::copy_nonoverlapping(
                elf_start.as_ptr::<u8>(),
                vaddr.as_mut_ptr(),
                filesz as usize,
            )
        }
        unsafe {
            core::ptr::write_bytes(
                (vaddr + filesz).as_mut_ptr::<u8>(),
                0,
                (memsz - filesz) as usize,
            )
        };

        highest_page = Some(highest_page.map_or(pages.end, |old| {
            if old.start_address() > pages.end.start_address() {
                old
            } else {
                pages.end
            }
        }))
    }

    let highest_page = highest_page.unwrap();
    let stack_bottom = highest_page.start_address() + Size4KiB::SIZE;
    let stack = unsafe {
        stack::create_stack_at(
            stack_bottom,
            &mut info.page_table,
            &mut info.frame_allocator,
            PageTableFlags::USER_ACCESSIBLE,
        )
    };

    println!("Setup stack {stack:?}");

    let entry = VirtAddr::new(elf.elf_header().e_entry(LittleEndian));
    println!("ELF loaded with entry point {:p}", entry);

    LoadedProgram { stack, entry }
}

pub const fn load_example_elf() -> &'static [u8] {
    TEST
}
