use core::{alloc::Layout, ops::DerefMut};

use addr2line::Context;
use alloc::{boxed::Box, collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
use kernel_utils::maybe_boxed::MaybeBoxed;
use log::{debug, info, warn};
use object::{
    LittleEndian,
    read::elf::{ElfFile64, FileHeader, ProgramHeader},
};
use ouroboros::self_referencing;
use spin::Once;
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, FrameDeallocator, Mapper, Page, PageSize, PageTableFlags, Size4KiB,
        mapper::CleanUp,
    },
};

use crate::{
    dwarf::{EndianSlice, load_dwarf},
    multitask::lock::ReentrantMutex,
    setup::KERNEL_INFO,
    stack::{self, GeneralStack},
    unwind::eh::EhInfo,
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

#[self_referencing]
pub struct ElfWithDataAndDwarf {
    data: Box<[u8]>,
    #[borrows(data)]
    #[covariant]
    elf: SystemElf<'this>,
    #[borrows(elf)]
    #[not_covariant]
    addr2line: Once<Option<Arc<ReentrantMutex<Context<EndianSlice<'this>>>>>>,
    #[borrows(elf)]
    #[not_covariant]
    eh_info: Once<Option<EhInfo<'this>>>,
}

pub struct LoadedProgram {
    // TODO mem
    load_offset: u64,
    stack: GeneralStack,
    entry: VirtAddr,
    elf: ElfWithDataAndDwarf,
    mapped_pages: BTreeMap<Page, PageTableFlags>,
}

impl core::fmt::Debug for LoadedProgram {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LoadedProgram")
            .field("load_offset", &self.load_offset)
            .field("stack", &self.stack)
            .field("entry", &self.entry)
            .finish()
    }
}

impl LoadedProgram {
    pub const fn entry(&self) -> VirtAddr {
        self.entry
    }

    pub const fn stack(&self) -> &GeneralStack {
        &self.stack
    }

    pub fn elf(&self) -> &SystemElf<'_> {
        self.elf.borrow_elf()
    }

    pub fn with_addr2line<T, F: FnOnce(&Context<EndianSlice<'_>>) -> T>(&self, f: F) -> Option<T> {
        self.elf.with(|x| {
            let r = x.addr2line.call_once(|| {
                let dwarf = load_dwarf(x.elf).inspect_err(|e| warn!("{e:?}")).ok()?;

                info!("Loaded DWARF for process");

                let c = Context::from_dwarf(dwarf)
                    .inspect_err(|e| warn!("{e:?}"))
                    .ok()
                    .map(ReentrantMutex::new)
                    .map(Arc::new);

                info!("Loaded addr2line context for process");
                c
            });

            r.as_ref().map(|x| {
                let lock = x.lock();
                let res = f(&lock);
                drop(lock);
                res
            })
        })
    }

    pub fn eh_info(&self) -> Option<&EhInfo<'_>> {
        self.elf.with(|x| {
            x.eh_info
                .call_once(|| {
                    let e = EhInfo::from_elf(x.elf, self.load_offset);
                    info!("Loaded eh_info for process");
                    e
                })
                .as_ref()
        })
    }

    pub const fn load_offset(&self) -> u64 {
        self.load_offset
    }

    /// # Safety
    /// The program cant be returned to later, no pages mapped for this should be accessed after the unload
    /// and the page table used must be the one used to map the pages
    pub unsafe fn unload(self) {
        // Unload the stack
        let mut lock = KERNEL_INFO.get().unwrap().alloc_kinf.lock();
        let mem = &mut *lock;
        unsafe { stack::clear_stack(self.stack, &mut mem.page_table, &mut mem.frame_allocator) };

        let mut min_max = None;
        for page in self.mapped_pages.keys() {
            let (frame, flush) = mem.page_table.unmap(*page).expect("Mapped page");
            flush.flush();
            unsafe { mem.frame_allocator.deallocate_frame(frame) };
            if let Some((min, max)) = &mut min_max {
                if *min > *page {
                    *min = *page
                } else if *max < *page {
                    *max = *page
                }
            } else {
                min_max = Some((*page, *page));
            }
        }

        if let Some((min, max)) = min_max {
            unsafe {
                mem.page_table
                    .clean_up_addr_range(Page::range_inclusive(min, max), &mut mem.frame_allocator)
            };
        }

        drop(lock);
    }
}

pub fn load_elf(bytes: &[u8]) -> LoadedProgram {
    let align = core::mem::align_of::<object::elf::FileHeader64<object::endian::LittleEndian>>();

    let aligned = realign_if_necessary(align, bytes);

    let elf_contained: ElfWithDataAndDwarf = ElfWithDataAndDwarfBuilder {
        data: aligned.into_owned(),
        elf_builder: |x| SystemElf::parse(x).expect("Correct ELF"),
        addr2line_builder: |_| Once::new(),
        eh_info_builder: |_| Once::new(),
    }
    .build();

    let elf = elf_contained.borrow_elf();
    // TODO check if its executable

    // TODO verify general header

    let offset = 0;

    debug!("ELF type: {:x}", elf.elf_header().e_type(LittleEndian));

    // Show prog headers
    let mut loads = Vec::<((u64, u64), (VirtAddr, u64), ElfPhSegmentFlags)>::new();
    debug!(
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
        debug!(
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
    let mut info_lock = info.alloc_kinf.lock();
    let info = info_lock.deref_mut();
    let mut highest_page: Option<Page> = None;
    let mut mapped_pages = BTreeMap::new();
    for ((offset, filesz), (vaddr, memsz), flags) in loads {
        debug!(
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
                debug!("Skipping {p:?}. Already mapped with flags {x:?}");
                if page_flags != *x {
                    warn!(
                        "Unexpected page flags ({page_flags:?}) different from already mapped ({x:?})"
                    )
                }
            } else {
                debug!("Mapping {p:?} with flags {page_flags:?}");
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
        let elf_start = VirtAddr::from_ptr(elf_contained.borrow_data().as_ptr()) + offset;
        debug!("Copying from {elf_start:p} to {vaddr:p} {filesz:x} bytes");
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

    drop(info_lock);

    debug!("Setup stack {stack:?}");

    let entry = VirtAddr::new(elf.elf_header().e_entry(LittleEndian));
    info!("ELF loaded with entry point {:p}", entry);

    LoadedProgram {
        stack,
        entry,
        elf: elf_contained,
        load_offset: offset,
        mapped_pages,
    }
}

pub const fn load_example_elf() -> &'static [u8] {
    TEST
}
