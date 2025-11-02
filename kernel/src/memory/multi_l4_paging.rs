use alloc::{sync::Arc, vec::Vec};
use kernel_utils::smallmap::SmallBTreeMap;
use log::{debug, info, trace};
use x86_64::{
    VirtAddr,
    structures::paging::{
        Mapper, OffsetPageTable, Page, PageTable, PageTableIndex, PhysFrame, Size4KiB, Translate,
        mapper::CleanUp, page::PageRangeInclusive,
    },
};

const KERNEL_P4_START: u16 = 1; // adjust: index where higher-half begins

#[derive(Debug)]
pub struct PageTableToken {
    #[allow(unused)]
    inner: PhysFrame,
}

struct PageTableInfo {
    addr: VirtAddr,
    token: Option<Arc<PageTableToken>>,
}

pub struct PageTables {
    current: OffsetPageTable<'static>,
    current_frame: PhysFrame,
    l4_tables: SmallBTreeMap<1, PhysFrame, PageTableInfo>,
}

impl PageTables {
    pub fn new(current: OffsetPageTable<'static>) -> Self {
        let (phys_f, _) = x86_64::registers::control::Cr3::read();
        Self {
            l4_tables: {
                let mut map = SmallBTreeMap::new();

                let virt_addr = VirtAddr::from_ptr(current.level_4_table());
                debug_assert_eq!(
                    current.phys_offset() + phys_f.start_address().as_u64(),
                    virt_addr,
                    "Current CR3 does not match current Page Table"
                );
                map.insert(
                    phys_f,
                    PageTableInfo {
                        addr: virt_addr,
                        token: None,
                    },
                );

                map
            },
            current_frame: phys_f,
            current,
        }
    }

    pub fn set_current_page_table<A>(&mut self, frame_alloc: &mut A)
    where
        A: x86_64::structures::paging::FrameDeallocator<Size4KiB> + ?Sized,
    {
        let (frame, _) = x86_64::registers::control::Cr3::read();
        self.set_current_page_table_frame(&frame, frame_alloc);
    }

    fn set_current_page_table_frame<A>(&mut self, frame: &PhysFrame, frame_alloc: &mut A)
    where
        A: x86_64::structures::paging::FrameDeallocator<Size4KiB> + ?Sized,
    {
        x86_64::instructions::interrupts::without_interrupts(|| {
            let old = self.l4_tables.get(&self.current_frame).unwrap();
            let old_frame = self.current_frame;
            let old_refs = old.token.as_ref().map(Arc::strong_count);
            info!("Old frame ({:?}) refs: {:?}", self.current_frame, old_refs);
            let addr = self
                .l4_tables
                .get(frame)
                .expect("the CR3 page table to be registered");
            self.current_frame = *frame;
            info!("Switching to page table with frame {frame:?}");
            self.current = unsafe {
                OffsetPageTable::<'static>::new(
                    addr.addr.as_mut_ptr::<PageTable>().as_mut().unwrap(),
                    self.current.phys_offset(),
                )
            };
            if old_refs == Some(1) && frame.start_address() != old_frame.start_address() {
                // old frame is unused and we switched to something else
                info!("Old CR3 is unused, cleaning up");
                let _ = self.l4_tables.remove(&old_frame).unwrap();
                // No need to unmap the page as we're accessing the frame through the memory mapping
                unsafe { frame_alloc.deallocate_frame(old_frame) };
            }
        });
    }

    unsafe fn switch_to_frame(frame: PhysFrame) {
        let (_, flags) = x86_64::registers::control::Cr3::read();
        unsafe {
            x86_64::registers::control::Cr3::write(frame, flags);
        }
    }

    pub fn create_process_p4_and_switch<A>(&mut self, frame_alloc: &mut A) -> Arc<PageTableToken>
    where
        A: x86_64::structures::paging::FrameAllocator<Size4KiB>
            + x86_64::structures::paging::FrameDeallocator<Size4KiB>
            + ?Sized,
    {
        let sp: u64;
        unsafe {
            core::arch::asm!("mov {0},rsp", lateout(reg) sp);
        }
        debug!("Creating process p4 to prepare for switch (current sp: {sp:x})");
        let (frame, token) = self
            .create_process_p4(frame_alloc)
            .expect("A frame for the l4 table");
        debug!("Created: {frame:?}");
        x86_64::instructions::interrupts::without_interrupts(|| {
            self.set_current_page_table_frame(&frame, frame_alloc);
            unsafe {
                Self::switch_to_frame(frame);
            }
        });
        token
    }

    fn create_process_p4<A>(
        &mut self,
        frame_alloc: &mut A,
    ) -> Option<(PhysFrame, Arc<PageTableToken>)>
    where
        A: x86_64::structures::paging::FrameAllocator<Size4KiB> + ?Sized,
    {
        debug!("Allocating frame for new p4");
        let frame = frame_alloc.allocate_frame()?;
        debug!("Allocated frame: {frame:?}");

        let offset = self.current.phys_offset();

        let page_addr = offset + frame.start_address().as_u64();
        debug!("Getting a pointer to the frame virtaddr({page_addr:p})");
        let page_table = unsafe { page_addr.as_mut_ptr::<PageTable>().as_mut() }.unwrap();
        *page_table = PageTable::new(); // initialize it
        debug!("Initialized current p4 table here -> virtaddr({page_addr:p})");
        for (a, b) in page_table
            .iter_mut()
            .zip(self.current.level_4_table().iter())
            .skip(KERNEL_P4_START as usize)
        {
            *a = b.clone();
        }
        info!("Copied current p4 table here -> virtaddr({page_addr:p})");
        let token = Arc::new(PageTableToken { inner: frame });

        self.l4_tables.insert(
            frame,
            PageTableInfo {
                addr: page_addr,
                token: Some(token.clone()),
            },
        );

        Some((frame, token))
    }

    fn all_but_current_internal<'a>(
        tables: impl Iterator<Item = (&'a PhysFrame, &'a PageTableInfo)>,
        frame: &'a PhysFrame,
    ) -> impl Iterator<Item = &'a mut PageTable> {
        tables
            .filter(move |(i, _)| **i != *frame)
            .map(|(_, x)| unsafe { x.addr.as_mut_ptr::<PageTable>().as_mut() }.unwrap())
    }

    #[allow(clippy::needless_pass_by_ref_mut)]
    fn all_but_current(&mut self) -> impl Iterator<Item = &mut PageTable> {
        Self::all_but_current_internal(self.l4_tables.iter(), &self.current_frame)
    }
}

impl CleanUp for PageTables {
    unsafe fn clean_up<D>(&mut self, frame_deallocator: &mut D)
    where
        D: x86_64::structures::paging::FrameDeallocator<x86_64::structures::paging::Size4KiB>,
    {
        unsafe {
            self.clean_up_addr_range(
                PageRangeInclusive {
                    start: Page::from_start_address(VirtAddr::new(0)).unwrap(),
                    end: Page::from_start_address(VirtAddr::new(0xffff_ffff_ffff_f000)).unwrap(),
                },
                frame_deallocator,
            )
        }
    }

    unsafe fn clean_up_addr_range<D>(
        &mut self,
        range: x86_64::structures::paging::page::PageRangeInclusive,
        frame_deallocator: &mut D,
    ) where
        D: x86_64::structures::paging::FrameDeallocator<x86_64::structures::paging::Size4KiB>,
    {
        unsafe {
            self.current.clean_up_addr_range(range, frame_deallocator);
        }
        let current = self
            .current
            .level_4_table()
            .iter()
            .skip(KERNEL_P4_START as usize);
        // Iterator over the iterators of entries of each table [table: [entries]]
        // SHould be an iterator over the iterator of each table, for each entry [entries: [tables]]
        let mut other: Vec<_> =
            Self::all_but_current_internal(self.l4_tables.iter(), &self.current_frame)
                .map(|x| x.iter_mut().skip(KERNEL_P4_START as usize))
                .collect();

        for e in current {
            for t in &mut other {
                let other_entry = t.next().unwrap();
                other_entry.clone_from(e);
            }
        }
    }
}

impl Mapper<Size4KiB> for PageTables {
    unsafe fn map_to_with_table_flags<A>(
        &mut self,
        page: x86_64::structures::paging::Page<Size4KiB>,
        frame: x86_64::structures::paging::PhysFrame<Size4KiB>,
        flags: x86_64::structures::paging::PageTableFlags,
        parent_table_flags: x86_64::structures::paging::PageTableFlags,
        frame_allocator: &mut A,
    ) -> Result<
        x86_64::structures::paging::mapper::MapperFlush<Size4KiB>,
        x86_64::structures::paging::mapper::MapToError<Size4KiB>,
    >
    where
        Self: Sized,
        A: x86_64::structures::paging::FrameAllocator<Size4KiB> + ?Sized,
    {
        let flush = unsafe {
            self.current.map_to_with_table_flags(
                page,
                frame,
                flags,
                parent_table_flags,
                frame_allocator,
            )
        }?;

        let p4_index = page.p4_index();

        if p4_index >= PageTableIndex::new(KERNEL_P4_START) {
            // println!("Created mapping in kernelspace (P4 idx: {p4_index:?} - {page:?})");
            let current_e = &self.current.level_4_table()[p4_index];
            // Copy kernel tables
            for e in Self::all_but_current_internal(self.l4_tables.iter(), &self.current_frame) {
                e[p4_index].clone_from(current_e);
            }
        } else {
            trace!(
                "Created mapping in userspace (Current frame: {:?} / P4 idx: {p4_index:?} - {page:?})",
                self.current_frame
            )
        }

        Ok(flush)
    }

    fn unmap(
        &mut self,
        page: x86_64::structures::paging::Page<Size4KiB>,
    ) -> Result<
        (
            x86_64::structures::paging::PhysFrame<Size4KiB>,
            x86_64::structures::paging::mapper::MapperFlush<Size4KiB>,
        ),
        x86_64::structures::paging::mapper::UnmapError,
    > {
        self.current.unmap(page) // Nothing needs to be done, no cleanup is performed
    }

    unsafe fn update_flags(
        &mut self,
        page: x86_64::structures::paging::Page<Size4KiB>,
        flags: x86_64::structures::paging::PageTableFlags,
    ) -> Result<
        x86_64::structures::paging::mapper::MapperFlush<Size4KiB>,
        x86_64::structures::paging::mapper::FlagUpdateError,
    > {
        unsafe { self.current.update_flags(page, flags) }
    }

    unsafe fn set_flags_p4_entry(
        &mut self,
        page: x86_64::structures::paging::Page<Size4KiB>,
        flags: x86_64::structures::paging::PageTableFlags,
    ) -> Result<
        x86_64::structures::paging::mapper::MapperFlushAll,
        x86_64::structures::paging::mapper::FlagUpdateError,
    > {
        let flush = unsafe { self.current.set_flags_p4_entry(page, flags) }?;

        let p4_index = page.p4_index();

        if p4_index >= PageTableIndex::new(KERNEL_P4_START) {
            for p4 in self.all_but_current() {
                let p4_entry = &mut p4[p4_index];

                p4_entry.set_flags(flags);
            }
        }

        Ok(flush)
    }

    unsafe fn set_flags_p3_entry(
        &mut self,
        page: x86_64::structures::paging::Page<Size4KiB>,
        flags: x86_64::structures::paging::PageTableFlags,
    ) -> Result<
        x86_64::structures::paging::mapper::MapperFlushAll,
        x86_64::structures::paging::mapper::FlagUpdateError,
    > {
        unsafe { self.current.set_flags_p3_entry(page, flags) }
    }

    unsafe fn set_flags_p2_entry(
        &mut self,
        page: x86_64::structures::paging::Page<Size4KiB>,
        flags: x86_64::structures::paging::PageTableFlags,
    ) -> Result<
        x86_64::structures::paging::mapper::MapperFlushAll,
        x86_64::structures::paging::mapper::FlagUpdateError,
    > {
        unsafe { self.current.set_flags_p2_entry(page, flags) }
    }

    fn translate_page(
        &self,
        page: x86_64::structures::paging::Page<Size4KiB>,
    ) -> Result<
        x86_64::structures::paging::PhysFrame<Size4KiB>,
        x86_64::structures::paging::mapper::TranslateError,
    > {
        self.current.translate_page(page)
    }
}

impl Translate for PageTables {
    fn translate(
        &self,
        addr: x86_64::VirtAddr,
    ) -> x86_64::structures::paging::mapper::TranslateResult {
        self.current.translate(addr)
    }
}
