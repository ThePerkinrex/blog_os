use core::{
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

use addr2line::Context;
use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
use kernel_utils::aligned_bytes::{AlignedBytes, realign_if_necessary};
use log::{debug, info, warn};
use object::{
    LittleEndian, Object, ObjectSymbol, ObjectSymbolTable,
    read::elf::{ElfFile64, FileHeader, ProgramHeader},
};
use ouroboros::self_referencing;
use spin::Once;
use thiserror::Error;
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, FrameDeallocator, Mapper, Page, PageSize, PageTableFlags, Size4KiB,
        mapper::CleanUp,
    },
};

use crate::{
    dwarf::{EndianSlice, load_dwarf},
    elf::symbol::SymbolResolver,
    multitask::lock::ReentrantMutex,
    setup::KERNEL_INFO,
    stack::{self, GeneralStack},
    unwind::eh::EhInfo,
};

pub mod symbol;

pub type SystemElf<'a> = ElfFile64<'a, LittleEndian, &'a [u8]>;

// fn copy_aligned_box(align: usize, og: &[u8]) -> Box<[u8]> {
//     let size = og.len();
//     let layout = Layout::from_size_align(size, align).unwrap();
//     unsafe {
//         let ptr = alloc::alloc::alloc(layout);
//         if ptr.is_null() {
//             panic!("allocation failed");
//         }
//         let slice = core::slice::from_raw_parts_mut(ptr, size);
//         slice.copy_from_slice(og);
//         Box::from_raw(slice)
//     }
// }

// fn realign_if_necessary<'a>(align: usize, og: &'a [u8]) -> MaybeBoxed<'a, [u8]> {
//     if (og.as_ptr() as usize).is_multiple_of(align) {
//         MaybeBoxed::Borrowed(og)
//     } else {
//         MaybeBoxed::Boxed(copy_aligned_box(align, og))
//     }
// }

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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum EType {
    ET_NONE = 0, // An unknown type.
    ET_REL,      // A relocatable file.
    ET_EXEC,     // An executable file.
    ET_DYN,      // A shared object.
    ET_CORE,     // A core file.
}

impl From<u16> for EType {
    fn from(value: u16) -> Self {
        match value {
            1 => Self::ET_REL,
            2 => Self::ET_EXEC,
            3 => Self::ET_DYN,
            4 => Self::ET_CORE,
            _ => Self::ET_NONE,
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
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
    data: AlignedBytes,
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

pub struct UserHeap {
    size: u64,
    brk: VirtAddr,
    mapped_pages: BTreeMap<Page, PageTableFlags>,
}

impl UserHeap {
    pub const fn new(brk: VirtAddr) -> Self {
        Self {
            size: 0,
            brk,
            mapped_pages: BTreeMap::new(),
        }
    }

    pub fn change_brk(&mut self, stack: &GeneralStack, offset: i64) -> Option<VirtAddr> {
        if offset == 0 {
            Some(self.brk)
        } else if offset < 0 {
            let new_brk = (self.brk - offset.unsigned_abs()).align_up(Size4KiB::SIZE);
            if new_brk == self.brk {
                Some(self.brk)
            } else {
                todo!("implement brk shinking (0x{offset:x} - {offset})")
            }
        } else {
            let new_brk = (self.brk + offset.unsigned_abs()).align_up(Size4KiB::SIZE);
            let new_pages = Page::<Size4KiB>::range(
                Page::containing_address(self.brk),
                Page::containing_address(new_brk),
            );
            if new_pages.end >= Page::containing_address(stack.bottom()) {
                panic!("Memory overflow, cannot allocate more heap: {new_pages:?} -> {stack:?}")
            }

            let info = KERNEL_INFO.get().unwrap();
            let mut info_lock = info.alloc_kinf.lock();
            let info = info_lock.deref_mut();

            let page_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE; // For now with one page table we should be able to write to it

            for p in new_pages {
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
                self.mapped_pages.insert(p, page_flags);
            }

            drop(info_lock);

            let growth = new_brk - self.brk;
            self.size += growth;

            self.brk = new_brk;
            Some(self.brk)
        }
    }
}

impl Drop for UserHeap {
    fn drop(&mut self) {
        // Unload the stack
        let mut lock = KERNEL_INFO.get().unwrap().alloc_kinf.lock();
        let mem = &mut *lock;

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

pub struct LoadedElf<S: SymbolResolver> {
    load_offset: u64,
    elf: ElfWithDataAndDwarf,
    mapped_pages: BTreeMap<Page, ElfPhSegmentFlags>,
    highest_page: Option<Page>,
    _symbol_resolver: S,
}

impl<S: SymbolResolver> core::fmt::Debug for LoadedElf<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LoadedElf")
            .field("load_offset", &self.load_offset)
            .finish()
    }
}

impl<S: SymbolResolver> LoadedElf<S> {
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

    //     /// # Safety
    //     /// The program cant be returned to later, no pages mapped for this should be accessed after the unload
    //     /// and the page table used must be the one used to map the pages
    //     pub unsafe fn unload(self) {

    //         self.symbol_resolver.unload();
    //     }
}

impl<S: SymbolResolver> Drop for LoadedElf<S> {
    fn drop(&mut self) {
        // Unload the stack
        let mut lock = KERNEL_INFO.get().unwrap().alloc_kinf.lock();
        let mem = &mut *lock;

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

pub struct LoadedProgram {
    elf: LoadedElf<()>,
    stack: ManuallyDrop<GeneralStack>,
    entry: VirtAddr,
    heap: ReentrantMutex<UserHeap>,
}

impl core::fmt::Debug for LoadedProgram {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LoadedProgram")
            .field("elf", &self.elf)
            .field("stack", &self.stack)
            .field("entry", &self.entry)
            .finish()
    }
}

impl LoadedProgram {
    pub const fn entry(&self) -> VirtAddr {
        self.entry
    }

    pub fn stack(&self) -> &GeneralStack {
        &self.stack
    }

    pub const fn heap(&self) -> &ReentrantMutex<UserHeap> {
        &self.heap
    }
}

impl Drop for LoadedProgram {
    fn drop(&mut self) {
        let stack = unsafe { ManuallyDrop::take(&mut self.stack) };

        // Unload the stack
        let mut lock = KERNEL_INFO.get().unwrap().alloc_kinf.lock();
        let mem = &mut *lock;
        unsafe { stack::clear_stack(stack, &mut mem.page_table, &mut mem.frame_allocator) };

        drop(lock);
    }
}

impl Deref for LoadedProgram {
    type Target = LoadedElf<()>;

    fn deref(&self) -> &Self::Target {
        &self.elf
    }
}

impl DerefMut for LoadedProgram {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.elf
    }
}

#[derive(Debug, Error)]
pub enum ElfLoadError {
    #[error("Invalid type: {0:?}")]
    InvalidType(EType),
    #[error("Unable to allocate memory region for loading this ELF")]
    MemAllocError,
}

pub type ElfHeader = object::elf::FileHeader64<object::endian::LittleEndian>;

pub const ELF_ALIGN: usize = core::mem::align_of::<ElfHeader>();

pub fn load_elf<S: SymbolResolver>(
    bytes: &[u8],
    type_check: impl FnOnce(&EType, u64) -> Result<VirtAddr, ElfLoadError>,
    user: bool,
    mut resolver: S,
) -> Result<LoadedElf<S>, ElfLoadError> {
    let aligned = realign_if_necessary::<ElfHeader>(bytes);

    let elf_contained: ElfWithDataAndDwarf = ElfWithDataAndDwarfBuilder {
        data: aligned.into_owned(),
        elf_builder: |x| SystemElf::parse(x).expect("Correct ELF"),
        addr2line_builder: |_| Once::new(),
        eh_info_builder: |_| Once::new(),
    }
    .build();

    let elf = elf_contained.borrow_elf();

    let e_type: EType = elf.elf_header().e_type(LittleEndian).into();

    debug!("ELF type: {:?} ({:x})", e_type, e_type as u16);

    let mut loads = Vec::<((u64, u64), (u64, u64), ElfPhSegmentFlags)>::new();
    debug!(
        "                type      flags     offset      vaddr      paddr     filesz      memsz      align"
    );
    let mut highest_end = 0;
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
            highest_end = highest_end.max(vaddr + memsz);
            loads.push(((offset, filesz), (vaddr, memsz), flags));
        }
    }

    let base_addr = type_check(&e_type, highest_end)?;

    let mut base_flags = PageTableFlags::PRESENT; // To setup the pages, we need to write to them

    if user {
        base_flags |= PageTableFlags::USER_ACCESSIBLE;
    }

    let base_setup_flags = base_flags | PageTableFlags::WRITABLE; // To setup the pages, we need to write to them

    fn get_flags(page_flags: &mut PageTableFlags, flags: ElfPhSegmentFlags) {
        if !flags.contains(ElfPhSegmentFlags::E) {
            *page_flags |= PageTableFlags::NO_EXECUTE
        }
        if flags.contains(ElfPhSegmentFlags::W) {
            *page_flags |= PageTableFlags::WRITABLE
        }
    }

    let info = KERNEL_INFO.get().unwrap();
    let mut info_lock = info.alloc_kinf.lock();
    let info = info_lock.deref_mut();
    let mut highest_page: Option<Page> = None;
    let mut mapped_pages = BTreeMap::new();
    for ((offset, filesz), (vaddr_offset, memsz), flags) in loads {
        let vaddr = base_addr + vaddr_offset;
        debug!(
            "Loading segment at offset {offset:x} (sz: {filesz:x}) to {vaddr:p} (sz: {memsz:x})"
        );
        let pages = Page::<Size4KiB>::range_inclusive(
            Page::containing_address(vaddr),
            Page::containing_address(vaddr + memsz),
        );
        for p in pages {
            let mut page_flags = base_setup_flags;

            if let Some(x) = mapped_pages.get(&p) {
                debug!("Page {p:?} already mapped with flags {x:?} (new: {flags:?})");
                let new_flags = flags | *x;
                get_flags(&mut page_flags, new_flags);
                // if !new_flags.contains(ElfPhSegmentFlags::E) {
                //     page_flags |= PageTableFlags::NO_EXECUTE
                // }
                // if new_flags.contains(ElfPhSegmentFlags::W) {
                //     page_flags |= PageTableFlags::WRITABLE
                // }
                // if new_flags.contains(ElfPhSegmentFlags::R) {
                //     page_flags |= PageTableFlags::USER_ACCESSIBLE
                // }

                unsafe { info.page_table.update_flags(p, page_flags) }
                    .unwrap()
                    .flush();
            } else {
                get_flags(&mut page_flags, flags);
                // if !flags.contains(ElfPhSegmentFlags::E) {
                //     page_flags |= PageTableFlags::NO_EXECUTE
                // }
                // if flags.contains(ElfPhSegmentFlags::W) {
                //     page_flags |= PageTableFlags::WRITABLE
                // }
                // if flags.contains(ElfPhSegmentFlags::R) {
                //     page_flags |= PageTableFlags::USER_ACCESSIBLE
                // }
                debug!("Mapping {p:?} with flags {page_flags:?}");
                let frame = info.frame_allocator.allocate_frame().expect("A frame");
                unsafe {
                    info.page_table
                        .map_to(p, frame, page_flags, &mut info.frame_allocator)
                }
                .unwrap()
                .flush();
                mapped_pages.insert(p, flags);
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

    // Remove writable flag for non writable pages
    for (page, flags) in mapped_pages.iter() {
        let mut page_flags = base_flags;
        get_flags(&mut page_flags, *flags);

        unsafe { info.page_table.update_flags(*page, page_flags) }
            .unwrap()
            .flush();
    }

    info!("Loaded segments");

    if e_type == EType::ET_DYN {
        let dynamic_symbol_table = elf.dynamic_symbol_table();
        for (addr, reloc) in elf.dynamic_relocations().into_iter().flatten() {
            match reloc.target() {
                object::RelocationTarget::Symbol(symbol_index) => {
                    debug!("RELOC {addr:x} {reloc:?}");
                    // for sym in elf.symbols() {
                    //     debug!("SYM {:?} {:?}", sym.index(), sym.name());
                    // }
                    // for sym in elf.dynamic_symbols() {
                    //     debug!("DYNSYM {:?} {:?}", sym.index(), sym.name());
                    // }
                    // for sym in elf.imports().unwrap() {
                    //     debug!("IMPORT {:?}", sym.name());
                    // }
                    // for sym in elf.exports().unwrap() {
                    //     debug!("EXPORT {:?}", sym.name());
                    // }
                    let symbol = dynamic_symbol_table
                        .unwrap()
                        .symbol_by_index(symbol_index)
                        .unwrap();
                    let value = resolver.resolve(symbol).unwrap();
                    let addr = base_addr + addr;

                    // unsafe { core::ptr::copy(value.data.as_ptr(), addr.as_mut_ptr::<u8>(), value.data.len()) };
                    unsafe {
                        addr.as_mut_ptr::<u64>().write_volatile(value.data.as_u64());
                    }

                    debug!(
                        "symbol {symbol_index:?} = {:?} (resolved: {value:?}, set {addr:p})",
                        symbol.name()
                    )
                }
                object::RelocationTarget::Section(section_index) => {
                    debug!("RELOC {addr:x} {reloc:?}");
                    unimplemented!("section {section_index:?}")
                }
                object::RelocationTarget::Absolute => {
                    let addr = base_addr + addr;
                    let value = base_addr.as_u64().saturating_add_signed(reloc.addend());

                    debug!("Setting {addr:p} to {value}");
                    unsafe { addr.as_mut_ptr::<u64>().write(value) };
                }
                x => unimplemented!("{x:?}"),
            }
        }
    }

    drop(info_lock);

    Ok(LoadedElf {
        load_offset: base_addr.as_u64(),
        elf: elf_contained,
        mapped_pages,
        highest_page,
        _symbol_resolver: resolver,
    })
}

pub fn load_user_program(bytes: &[u8]) -> LoadedProgram {
    let loaded_elf = load_elf(
        bytes,
        |e_type, _size| {
            if *e_type == EType::ET_DYN {
                // TODO randomize base address
                Ok(VirtAddr::zero() + Size4KiB::SIZE) // Skip first page
            } else if *e_type == EType::ET_EXEC {
                Ok(VirtAddr::zero())
            } else {
                panic!("{e_type:?} is not a valid ELF executable")
            }
        },
        true,
        (),
    )
    .unwrap();

    let info = KERNEL_INFO.get().unwrap();
    let mut info_lock = info.alloc_kinf.lock();
    let info = info_lock.deref_mut();

    let highest_page = loaded_elf.highest_page.unwrap();
    let brk = highest_page.start_address() + Size4KiB::SIZE;

    let stack_top = qemu_common::KERNEL_START;
    let stack = unsafe {
        stack::create_stack_from_top(
            stack_top,
            &mut info.page_table,
            &mut info.frame_allocator,
            PageTableFlags::USER_ACCESSIBLE,
        )
    };

    drop(info_lock);

    debug!("Setup stack {stack:?}");

    let entry =
        VirtAddr::new(loaded_elf.load_offset + loaded_elf.elf().elf_header().e_entry(LittleEndian));
    info!("ELF loaded with entry point {:p}", entry);

    LoadedProgram {
        stack: ManuallyDrop::new(stack),
        entry,
        elf: loaded_elf,
        heap: ReentrantMutex::new(UserHeap::new(brk)),
    }
}
