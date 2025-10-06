use linked_list_allocator::LockedHeap;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, Page, PageSize, PageTableFlags, Size4KiB, mapper::MapToError,
};

use crate::memory::pages::VirtRegionAllocator;

// pub const HEAP_START: u64 = 0x_4444_4444_0000;
pub const HEAP_PAGES: u64 = 20;
pub const HEAP_SIZE: u64 = HEAP_PAGES * Size4KiB::SIZE; // 100 KiB

pub fn init_heap<const CAP: usize>(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    virt_region_allocator: &mut VirtRegionAllocator<CAP, Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let heap_start = virt_region_allocator
        .alloc_pages(HEAP_PAGES as usize)
        .expect("Heap region");
    // let heap_sheap_starttart = VirtAddr::new(HEAP_START);
    let page_range = {
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    unsafe {
        ALLOCATOR
            .lock()
            .init(heap_start.as_mut_ptr(), HEAP_SIZE as usize);
    }

    Ok(())
}

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[cfg(test)]
mod test {
    use alloc::{boxed::Box, vec::Vec};

    use crate::allocator::HEAP_SIZE;

    #[test_case]
    fn simple_allocation() {
        let heap_value_1 = Box::new(41);
        let heap_value_2 = Box::new(13);
        assert_eq!(*heap_value_1, 41);
        assert_eq!(*heap_value_2, 13);
    }

    #[test_case]
    fn large_vec() {
        let n = 1000;
        let mut vec = Vec::new();
        for i in 0..n {
            vec.push(i);
        }
        assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
    }

    #[test_case]
    fn many_boxes() {
        for i in 0..HEAP_SIZE {
            let x = Box::new(i);
            assert_eq!(*x, i);
        }
    }
}
