use alloc::vec::Vec;
use x86_64::{
    structures::paging::{
        mapper::CleanUp, page::PageRangeInclusive, Mapper, OffsetPageTable, Page, PageTable, PageTableIndex, PhysFrame, Size4KiB, Translate
    }, VirtAddr
};

const KERNEL_P4_START: u16 = 256; // adjust: index where higher-half begins

pub struct PageTables {
    current: OffsetPageTable<'static>,
    current_idx: usize,
    l4_tables: Vec<VirtAddr>,
}

impl PageTables {
    pub fn new(current: OffsetPageTable<'static>) -> Self {
        Self {
            l4_tables: alloc::vec![VirtAddr::from_ptr(current.level_4_table())],
            current_idx: 0,
            current,
        }
    }

	// pub fn create_process_p4_and_switch<A>(&mut self, frame_alloc: &mut A) where 
    //     A: x86_64::structures::paging::FrameAllocator<Size4KiB> + ?Sized {
	// 	let frame = self 

	// }

	fn create_process_p4<A>(&mut self, frame_alloc: &mut A) -> Option<PhysFrame> where 
        A: x86_64::structures::paging::FrameAllocator<Size4KiB> + ?Sized, {
		let frame = frame_alloc.allocate_frame()?;

		let offset = self.current.phys_offset();

		let page_addr = offset + frame.start_address().as_u64();
		let page_table = unsafe {page_addr.as_mut_ptr::<PageTable>().as_mut()}.unwrap();
		*page_table = PageTable::new(); // initialize it
		page_table.clone_from(self.current.level_4_table());

		self.l4_tables.push(page_addr);

		Some(frame)
	}

	#[allow(clippy::needless_pass_by_ref_mut)]
	fn all_but_current_internal(tables: &mut [VirtAddr], idx: usize) -> impl Iterator<Item = &mut PageTable> {
		tables
                .iter()
                .copied()
                .enumerate()
                .filter(move |(i, _)| *i != idx)
                .map(|(_, x)| unsafe { x.as_mut_ptr::<PageTable>().as_mut() }.unwrap())
	}

	fn all_but_current(&mut self) -> impl Iterator<Item = &mut PageTable> {
		Self::all_but_current_internal(&mut self.l4_tables, self.current_idx)
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
        let mut other: Vec<_> = Self::all_but_current_internal(&mut self.l4_tables, self.current_idx).map(|x| x.iter_mut())
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
		let flush = unsafe{self.current.map_to_with_table_flags(page, frame, flags, parent_table_flags, frame_allocator)}?;
        
		let p4_index = page.p4_index();

        if p4_index >= PageTableIndex::new(KERNEL_P4_START) {
			let current_e = &self.current.level_4_table()[p4_index];
			// Copy kernel tables
			for e in Self::all_but_current_internal(&mut self.l4_tables, self.current_idx) {
				e[p4_index].clone_from(current_e);
			}
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
            for p4 in self.all_but_current()
            {
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
