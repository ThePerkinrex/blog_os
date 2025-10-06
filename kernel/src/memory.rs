use core::ops::Range;
use core::slice::Iter;

use alloc::collections::vec_deque::VecDeque;
use bootloader_api::info::{MemoryRegion, MemoryRegionKind, MemoryRegions};
use x86_64::PhysAddr;
use x86_64::{VirtAddr, structures::paging::PageTable};

use x86_64::structures::paging::{
    FrameAllocator, FrameDeallocator, OffsetPageTable, PhysFrame, Size4KiB,
};

pub mod pages;

/// Initialize a new OffsetPageTable.
///
/// # Safety
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn init_page_tables(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }
}

/// Returns a mutable reference to the active level 4 table.
///
///
/// # Safety
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

type UnusedFramesIter = core::iter::Map<
    core::iter::FlatMap<
        core::iter::Map<
            core::iter::Filter<
                core::slice::Iter<'static, MemoryRegion>,
                fn(&&MemoryRegion) -> bool,
            >,
            fn(&MemoryRegion) -> Range<u64>,
        >,
        core::iter::StepBy<Range<u64>>,
        fn(Range<u64>) -> core::iter::StepBy<Range<u64>>,
    >,
    fn(u64) -> PhysFrame,
>;

pub struct BootInfoFrameAllocator {
    unused_unalloc: UnusedFramesIter,
    // Only initialized once the heap is up
    dealloc: Option<VecDeque<PhysFrame>>,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// # Safety
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryRegions) -> Self {
        // get usable regions from memory map
        let regions: Iter<'static, MemoryRegion> = memory_map.iter();
        let usable_regions =
            regions.filter::<fn(&&MemoryRegion) -> bool>(|r| r.kind == MemoryRegionKind::Usable);
        // map each region to its address range
        let addr_ranges =
            usable_regions.map::<_, fn(&MemoryRegion) -> Range<u64>>(|r| r.start..r.end);
        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges
            .flat_map::<_, fn(Range<u64>) -> core::iter::StepBy<Range<u64>>>(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        let unused_unalloc: UnusedFramesIter =
            frame_addresses.map::<_, fn(u64) -> PhysFrame>(|addr| {
                PhysFrame::containing_address(PhysAddr::new(addr))
            });

        Self {
            unused_unalloc,
            dealloc: None,
        }
    }
}

impl BootInfoFrameAllocator {
    // /// Returns an iterator over the usable frames specified in the memory map.
    // fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
    //     // get usable regions from memory map
    //     let regions: Iter<'static, MemoryRegion> = self.memory_map.iter();
    //     let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
    //     // map each region to its address range
    //     let addr_ranges = usable_regions.map(|r| r.start..r.end);
    //     // transform to an iterator of frame start addresses
    //     let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
    //     // create `PhysFrame` types from the start addresses
    //     frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    // }

    /// Called when the heap is usable
    pub fn heap_init(&mut self) {
        self.dealloc = Some(VecDeque::new())
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.dealloc
            .as_mut()
            .and_then(VecDeque::pop_front)
            .or_else(|| self.unused_unalloc.next())
    }
}

impl FrameDeallocator<Size4KiB> for BootInfoFrameAllocator {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame<Size4KiB>) {
        if let Some(dealloc) = self.dealloc.as_mut() {
            dealloc.push_back(frame);
        }
    }
}
