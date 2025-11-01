use log::info;
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, FrameDeallocator, Mapper, Page, PageSize, PageTableFlags, Size4KiB,
        Translate, mapper::CleanUp, page::PageRangeInclusive,
    },
};

use crate::memory::pages::VirtRegionAllocator;

const STACK_PAGES: usize = 8; // 4KiB * 8 = 32KiB stacks

type SlabBitmapBase = u64;

const SLAB_BITMAP_ENTRIES: usize = 4;
const ENTRY_BITS: usize = SlabBitmapBase::BITS as usize;
const SLAB_STACKS: usize = ENTRY_BITS * SLAB_BITMAP_ENTRIES; // 64 * 4 = 256
const SLAB_PAGES: usize = SLAB_STACKS * STACK_PAGES; // 256 * 32KiB = 8MiB

// const KERNEL_STACK_REGION_START: VirtAddr = VirtAddr::new_truncate(0xFFFF_FE00_0000_0000);

pub struct GeneralStack {
    pages: PageRangeInclusive, // start: VirtAddr,
                               // end: VirtAddr
}

impl GeneralStack {
    pub const fn bottom(&self) -> VirtAddr {
        self.pages.start.start_address()
    }

    pub fn top(&self) -> VirtAddr {
        self.pages.end.start_address() + self.pages.end.size()
    }
}

impl core::fmt::Debug for GeneralStack {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Stack")
            .field("bottom", &self.bottom())
            .field("top", &self.top())
            .finish()
    }
}

pub struct SlabStack {
    idx: usize,
    stack: GeneralStack,
}

impl core::fmt::Debug for SlabStack {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Stack")
            .field("bottom", &self.bottom())
            .field("top", &self.top())
            .finish()
    }
}

impl SlabStack {
    pub const fn bottom(&self) -> VirtAddr {
        self.stack.bottom()
    }

    pub fn top(&self) -> VirtAddr {
        self.stack.top()
    }
}

/// # Safety
/// Must ensure that the next STACK_PAGES (4) are unmapped
pub unsafe fn create_stack_at<M: Mapper<Size4KiB> + Translate, F: FrameAllocator<Size4KiB>>(
    bottom: VirtAddr,
    mapper: &mut M,
    frame_alloc: &mut F,
    extra_flags: PageTableFlags,
) -> GeneralStack {
    let stack_top = bottom + (STACK_PAGES as u64) * Size4KiB::SIZE;

    // Leave the *bottom* page unmapped as a guard page
    let guard_end = bottom + Size4KiB::SIZE;
    let start_page = Page::containing_address(guard_end);
    let end_page = Page::containing_address(stack_top - 1); // top-1 ensures inclusive mapping up to last real byte

    let page_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | extra_flags;

    let pages = Page::range_inclusive(start_page, end_page);

    for page in pages {
        // If the page is already mapped, this likely indicates a bug / overlap; bail out.
        if mapper.translate_addr(page.start_address()).is_some() {
            // Rollback: unmap pages we already mapped for this stack and mark free again.
            // For simplicity, here we panic — in production you might want a safer rollback.
            panic!(
                "Attempt to map stack page that is already mapped: {:#x}",
                page.start_address().as_u64()
            );
        }

        let frame = frame_alloc
            .allocate_frame()
            .expect("frame allocation failed while creating stack");
        unsafe {
            mapper
                .map_to(page, frame, page_flags, frame_alloc)
                .expect("map_to failed")
                .flush();
        }
    }

    GeneralStack { pages }
}

/// # Safety
/// The stack shouldn't be used, and the pages used up by it should be unmappable
pub unsafe fn clear_stack<M: Mapper<Size4KiB> + CleanUp, F: FrameDeallocator<Size4KiB>>(
    stack: GeneralStack,
    mapper: &mut M,
    frame_dealloc: &mut F,
) {
    let pages = stack.pages;
    for p in pages {
        info!("Freeing stack page at {pages:?}");
        let (frame, flush) = mapper.unmap(p).expect("page should be mapped");
        flush.flush();
        unsafe {
            frame_dealloc.deallocate_frame(frame);
        }
    }
    unsafe {
        mapper.clean_up_addr_range(pages, frame_dealloc);
    }
}

pub struct StackAlloc {
    stack_info: [SlabBitmapBase; SLAB_BITMAP_ENTRIES],
    region_start: VirtAddr,
}

impl StackAlloc {
    pub fn new<const CAP: usize>(region_alloc: &mut VirtRegionAllocator<CAP, Size4KiB>) -> Self {
        let region_start = region_alloc
            .alloc_pages(SLAB_PAGES)
            .expect("Available space for all entries");
        Self {
            stack_info: [0; SLAB_BITMAP_ENTRIES],
            region_start,
        }
    }

    /// Create a new kernel stack and map its pages (leaving a guard page at the top).
    /// Returns a `Stack` descriptor on success.
    pub fn create_stack<M: Mapper<Size4KiB> + Translate, F: FrameAllocator<Size4KiB>>(
        &mut self,
        mapper: &mut M,
        frame_alloc: &mut F,
    ) -> Option<SlabStack> {
        let stack_idx = self.get_free_stack()?;
        self.set_stack_status(stack_idx, true);

        // Compute page offset within the slab region.
        let page_off = stack_idx * STACK_PAGES;
        let stack_bottom = self.region_start + (page_off as u64) * Size4KiB::SIZE;

        let stack =
            unsafe { create_stack_at(stack_bottom, mapper, frame_alloc, PageTableFlags::empty()) };

        info!("Created stack: {stack:?}");
        // Return a Stack descriptor. You can expand this later (store VirtAddr top/bottom).
        Some(SlabStack {
            idx: stack_idx,
            stack,
        })
    }

    const fn get_stack_status(&self, idx: usize) -> bool {
        let array_idx = idx / ENTRY_BITS;
        let entry = self.stack_info[array_idx];
        (entry >> (idx % ENTRY_BITS) & 1) == 1
    }

    const fn set_stack_status(&mut self, idx: usize, in_use: bool) {
        let array_idx = idx / ENTRY_BITS;
        if in_use {
            self.stack_info[array_idx] |= 1 << (idx % ENTRY_BITS);
        } else {
            self.stack_info[array_idx] &= !(1 << (idx % ENTRY_BITS));
        }
    }

    fn get_free_stack(&self) -> Option<usize> {
        (0..SLAB_STACKS).find(|&idx| !self.get_stack_status(idx))
    }

    /// # Safety
    /// The stack shouldn't be used, and the pages used up by it should be unmappable
    pub unsafe fn free_stack<M: Mapper<Size4KiB> + CleanUp, F: FrameDeallocator<Size4KiB>>(
        &mut self,
        stack: SlabStack,
        mapper: &mut M,
        frame_dealloc: &mut F,
    ) {
        info!("Cleaning up stack: {stack:?} at idx {}", stack.idx);

        unsafe {
            clear_stack(stack.stack, mapper, frame_dealloc);
        }
        self.set_stack_status(stack.idx, false); // Allow this range to be used again
    }

    pub fn detect_guard_page_access(
        &self,
        accessed: VirtAddr,
        current_stack: &SlabStack,
    ) -> GuardPageInfo {
        let region_start_page = Page::<Size4KiB>::containing_address(self.region_start);
        let region_end_page = region_start_page + SLAB_PAGES as u64;
        if region_start_page.start_address() <= accessed
            && region_end_page.start_address() > accessed
        {
            let page_off = current_stack.idx * STACK_PAGES;
            let stack_bottom = self.region_start + (page_off as u64) * Size4KiB::SIZE;
            if stack_bottom <= accessed {
                let guard_page = Page::<Size4KiB>::containing_address(stack_bottom);

                if accessed < guard_page.start_address() + guard_page.size() {
                    GuardPageInfo::CurrentStackOverflow
                } else {
                    GuardPageInfo::CurrentStack
                }
            } else {
                let stack_aligned = accessed.align_down(Size4KiB::SIZE * STACK_PAGES as u64);
                let stack_offset = stack_aligned - self.region_start;
                let stack_idx = stack_offset as usize / STACK_PAGES;

                let status = self.get_stack_status(stack_idx);

                let page_off = stack_idx as u64 * STACK_PAGES as u64;
                let stack_bottom = self.region_start + page_off * Size4KiB::SIZE;

                let guard_page = Page::<Size4KiB>::containing_address(stack_bottom);

                if accessed < guard_page.start_address() + guard_page.size() {
                    GuardPageInfo::OtherGuardPage(stack_idx, status)
                } else {
                    GuardPageInfo::OtherStack(stack_idx, status)
                }
            }
        } else {
            GuardPageInfo::Unknown
        }
    }
}

pub enum GuardPageInfo {
    CurrentStackOverflow,
    CurrentStack,
    OtherGuardPage(usize, bool),
    OtherStack(usize, bool),
    Unknown,
}
