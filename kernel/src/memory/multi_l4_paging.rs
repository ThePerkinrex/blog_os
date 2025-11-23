use alloc::{sync::Arc, vec::Vec};
use kernel_utils::smallmap::SmallBTreeMap;
use log::{debug, info, trace, warn};
use x86_64::{
    VirtAddr,
    structures::paging::{
        Mapper, OffsetPageTable, Page, PageTable, PageTableIndex, PhysFrame, Size4KiB, Translate,
        mapper::CleanUp,
        page::PageRangeInclusive,
        page_table::{PageTableEntry, PageTableLevel},
    },
};

use crate::memory::free_tables::{FreeEntry, FreeTables};

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
    kernel_start: VirtAddr,
}

fn filter_entries<'a, 'b>(
    current: &OffsetPageTable<'b>,
    iter: impl Iterator<Item = &'a PageTableEntry>,
) -> impl Iterator<Item = (usize, &'a PageTableEntry, VirtAddr, &'a PageTable)> {
    iter.enumerate()
        .filter(|(_, entry)| !entry.is_unused())
        .map(|(idx, entry)| {
            let table_virt = current.phys_offset() + entry.addr().as_u64();
            (idx, entry, table_virt)
        })
        .filter_map(|(idx, entry, virt)| {
            unsafe { virt.as_ptr::<PageTable>().as_ref() }.map(|tbl| (idx, entry, virt, tbl))
        })
}

impl PageTables {
    pub fn new(current: OffsetPageTable<'static>, kernel_start: VirtAddr) -> Self {
        let (phys_f, _) = x86_64::registers::control::Cr3::read();
        trace!(event = "page_tables_new", subevent = "start", current_frame:? = phys_f; "Initializing page tables (current p4: {phys_f:?})");
        Self {
            l4_tables: {
                let mut map = SmallBTreeMap::new();

                let virt_addr = VirtAddr::from_ptr(current.level_4_table());
                debug_assert_eq!(
                    current.phys_offset() + phys_f.start_address().as_u64(),
                    virt_addr,
                    "Current CR3 does not match current Page Table"
                );

                // for (p4_index, p4_entry, p3_table_addr, p3_table) in
                //     filter_entries(&current, current.level_4_table().iter())
                // {
                //     trace!(event = "page_tables_new", subevent = "p3_table", p3_table_addr:?, p4_entry_addr:? = p4_entry.addr(); "P3 table at {p3_table_addr:?}");
                //     for (p3_index, p3_entry, p2_table_addr, p2_table) in
                //         filter_entries(&current, p3_table.iter())
                //     {
                //         trace!(event = "page_tables_new", subevent = "p2_table", p2_table_addr:?, p4_entry_addr:? = p4_entry.addr(), p3_entry_addr:? = p3_entry.addr(); "P2 table at {p2_table_addr:?}");
                //         for (p2_index, p2_entry, p1_table_addr, p1_table) in
                //             filter_entries(&current, p2_table.iter())
                //         {
                //             trace!(event = "page_tables_new", subevent = "p1_table", p1_table_addr:?, p4_entry_addr:? = p4_entry.addr(), p3_entry_addr:? = p3_entry.addr(), p2_entry_addr:? = p2_entry.addr(); "P1 table at {p1_table_addr:?}");
                //             for (p1_index, p1_entry) in
                //                 p1_table.iter().enumerate().filter(|(_, x)| !x.is_unused())
                //             {
                //                 let virt = VirtAddr::new(
                //                     ((p4_index as u64) << 39)
                //                         | ((p3_index as u64) << 30)
                //                         | ((p2_index as u64) << 21)
                //                         | ((p1_index as u64) << 12),
                //                 );
                //                 trace!(event = "page_tables_new", subevent = "p1_entry", p1_table_addr:?, p4_entry_addr:? = p4_entry.addr(), p3_entry_addr:? = p3_entry.addr(), p2_entry_addr:? = p2_entry.addr(), addr:? = p1_entry.addr(), page_addr:? = virt, flags:? = p1_entry.flags(); "P1 entry at {:?} for page {virt:?}", p1_entry.addr());
                //             }
                //         }
                //     }
                // }

                map.insert(
                    phys_f,
                    PageTableInfo {
                        addr: virt_addr,
                        token: None,
                    },
                );

                trace!(event = "page_tables_new", subevent = "end", current_frame:? = phys_f; "Initialized page tables (current p4: {phys_f:?})");

                map
            },
            current_frame: phys_f,
            current,
            kernel_start,
        }
    }

    pub fn mapped_pages_in_range(
        &self,
        start: VirtAddr,
        end: VirtAddr,
    ) -> impl Iterator<Item = Page<Size4KiB>> {
        let p4s = start.p4_index();
        let p4e = end.p4_index();
        let p3s = start.p3_index();
        let p3e = end.p3_index();
        let p2s = start.p2_index();
        let p2e = end.p2_index();
        let p1s = start.p1_index();
        let p1e = end.p1_index();

        filter_entries(&self.current, self.current.level_4_table().iter())
            .skip_while(move |(i, _, _, _)| PageTableIndex::new_truncate(*i as u16) < p4s)
            .take_while(move |(i, _, _, _)| PageTableIndex::new_truncate(*i as u16) <= p4e)
            .flat_map(move |(p4_idx, _, _, p3_table)| {
                let p4i = PageTableIndex::new_truncate(p4_idx as u16);
                filter_entries(&self.current, p3_table.iter())
                    .skip_while(move |(i, _, _, _)| {
                        let ii = PageTableIndex::new_truncate(*i as u16);
                        p4i == p4s && ii < p3s
                    })
                    .take_while(move |(i, _, _, _)| {
                        let ii = PageTableIndex::new_truncate(*i as u16);
                        if p4i == p4e { ii <= p3e } else { true }
                    })
                    .flat_map(move |(p3_idx, _, _, p2_table)| {
                        let p4i = PageTableIndex::new_truncate(p4_idx as u16);
                        let p3i = PageTableIndex::new_truncate(p3_idx as u16);
                        filter_entries(&self.current, p2_table.iter())
                            .skip_while(move |(i, _, _, _)| {
                                let ii = PageTableIndex::new_truncate(*i as u16);
                                p4i == p4s && p3i == p3s && ii < p2s
                            })
                            .take_while(move |(i, _, _, _)| {
                                let ii = PageTableIndex::new_truncate(*i as u16);
                                if p4i == p4e && p3i == p3e {
                                    ii <= p2e
                                } else {
                                    true
                                }
                            })
                            .flat_map(move |(p2_idx, _, _, p1_table)| {
                                let p4i = PageTableIndex::new_truncate(p4_idx as u16);
                                let p3i = PageTableIndex::new_truncate(p3_idx as u16);
                                let p2i = PageTableIndex::new_truncate(p2_idx as u16);

                                p1_table
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, e)| !e.is_unused())
                                    .skip_while(move |(i, _)| {
                                        let ii = PageTableIndex::new_truncate(*i as u16);
                                        p4i == p4s && p3i == p3s && p2i == p2s && ii < p1s
                                    })
                                    .take_while(move |(i, _)| {
                                        let ii = PageTableIndex::new_truncate(*i as u16);
                                        if p4i == p4e && p3i == p3e && p2i == p2e {
                                            ii <= p1e
                                        } else {
                                            true
                                        }
                                    })
                                    .map(move |(p1_idx, _)| {
                                        let virt = VirtAddr::new(
                                            ((p4_idx as u64) << 39)
                                                | ((p3_idx as u64) << 30)
                                                | ((p2_idx as u64) << 21)
                                                | ((p1_idx as u64) << 12),
                                        );

                                        Page::containing_address(virt)
                                    })
                                    .filter(move |page| {
                                        let va = page.start_address();
                                        va >= start && va < end
                                    })
                            })
                    })
            })
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
            info!(event = "frame_switch", subevent = "before_switch", old_frame:?, new_frame:? = frame, old_refs:?; "Old frame ({:?}) refs: {:?}", self.current_frame, old_refs);
            let addr = self
                .l4_tables
                .get(frame)
                .expect("the CR3 page table to be registered");
            self.current_frame = *frame;
            info!(event = "frame_switch", subevent = "after_switch", old_frame:?, new_frame:? = frame; "Switching to page table with frame {frame:?}");
            self.current = unsafe {
                OffsetPageTable::<'static>::new(
                    addr.addr.as_mut_ptr::<PageTable>().as_mut().unwrap(),
                    self.current.phys_offset(),
                )
            };
            if old_refs == Some(1) && frame.start_address() != old_frame.start_address() {
                // old frame is unused and we switched to something else
                info!(event = "frame_switch", subevent = "cleanup", old_frame:?, new_frame:? = frame; "Old CR3 is unused, cleaning up");
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
        debug!(event = "create_p4", subevent = "before_create", sp; "Creating process p4 to prepare for switch (current sp: {sp:x})");
        let (frame, token) = self
            .create_process_p4(frame_alloc)
            .expect("A frame for the l4 table");
        debug!(event = "create_p4", subevent = "after_create", sp, frame:?; "Created: {frame:?}");
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
        debug!(event = "create_p4_internal", subevent = "before_p4_frame"; "Allocating frame for new p4");
        let frame = frame_alloc.allocate_frame()?;
        debug!(event = "create_p4_internal", subevent = "after_p4_frame", frame:?; "Allocated frame: {frame:?}");

        let offset = self.current.phys_offset();

        let page_addr = offset + frame.start_address().as_u64();
        debug!(event = "create_p4_internal", subevent = "frame_virtaddr", frame:?, page_addr:?; "Getting a pointer to the frame virtaddr({page_addr:p})");
        let page_table = unsafe { page_addr.as_mut_ptr::<PageTable>().as_mut() }.unwrap();
        *page_table = PageTable::new(); // initialize it
        debug!(event = "create_p4_internal", subevent = "init_p4", frame:?, page_addr:?; "Initialized current p4 table here -> virtaddr({page_addr:p})");
        for (a, b) in page_table
            .iter_mut()
            .zip(self.current.level_4_table().iter())
            .skip(self.kernel_start.p4_index().into())
        {
            *a = b.clone();
        }
        info!(event = "create_p4_internal", subevent = "copy_p4", frame:?, page_addr:?; "Copied current p4 table here -> virtaddr({page_addr:p})");
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
            .skip(self.kernel_start.p4_index().into());
        // Iterator over the iterators of entries of each table [table: [entries]]
        // SHould be an iterator over the iterator of each table, for each entry [entries: [tables]]
        let mut other: Vec<_> =
            Self::all_but_current_internal(self.l4_tables.iter(), &self.current_frame)
                .map(|x| x.iter_mut().skip(self.kernel_start.p4_index().into()))
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
        let p4_index = page.p4_index();
        let flush = unsafe {
            self.current.map_to_with_table_flags(
                page,
                frame,
                flags,
                parent_table_flags,
                frame_allocator,
            )
        }.inspect_err(|e| {
            warn!(event = "map_page", subevent = "fail_map", current_frame:? = self.current_frame, frame:?, page:?, error:? = e;
                "Failed to map page (Current frame: {:?} / P4 idx: {p4_index:?} - {page:?}) to frame {frame:?} ({e:?})",
                self.current_frame
            )
        })?;

        if p4_index >= self.kernel_start.p4_index() {
            // println!("Created mapping in kernelspace (P4 idx: {p4_index:?} - {page:?})");
            let current_e = &self.current.level_4_table()[p4_index];
            // Copy kernel tables
            for e in Self::all_but_current_internal(self.l4_tables.iter(), &self.current_frame) {
                e[p4_index].clone_from(current_e);
            }
            // trace!(event = "map_page", subevent = "map_kernel", current_frame:? = self.current_frame, frame:?, page:?;
            //     "Created mapping in kernelspace (Current frame: {:?} / P4 idx: {p4_index:?} - {page:?}) to frame {frame:?}",
            //     self.current_frame
            // )
        } else {
            trace!(event = "map_page", subevent = "map_user", current_frame:? = self.current_frame, frame:?, page:?;
                "Created mapping in userspace (Current frame: {:?} / P4 idx: {p4_index:?} - {page:?}) to frame {frame:?}",
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
        trace!(event = "unmap_page", current_frame:? = self.current_frame, page:?;
            "Unmapping page: {page:?} ({:?})",
            self.current_frame
        );
        self.current.unmap(page) // Nothing needs to be done, no cleanup is performed, if at kernel level, no p3 tables are removed, and at user level, it doesnt matter 
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

        if p4_index >= self.kernel_start.p4_index() {
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

impl FreeTables for PageTables {
    fn free_l4_kernel_entries(&self) -> impl Iterator<Item = super::free_tables::FreeEntry> {
        self.current
            .level_4_table()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_unused())
            .map(|(i, _)| FreeEntry {
                index: PageTableIndex::new_truncate(i as u16),
                alignment: PageTableLevel::Four.entry_address_space_alignment(),
            })
    }
}
