use core::ops::DerefMut;
use spin::Mutex;
use talc::{OomHandler, Span, Talc, Talck};
use x86_64::structures::paging::{
    FrameAllocator, Mapper, Page, PageSize, PageTableFlags, Size4KiB, mapper::MapToError,
};

use crate::{memory::pages::VirtRegionAllocator, multitask::lock::ReentrantMutex, println, setup::{KERNEL_INFO, AllocKernelInfo}};

// pub const HEAP_START: u64 = 0x_4444_4444_0000;
pub const HEAP_PAGES: u64 = 1024;
pub const HEAP_SIZE: u64 = HEAP_PAGES * Size4KiB::SIZE; // 16 MiB

// TODO grow on oom

pub fn init_heap(mutable_inf: &'static ReentrantMutex<AllocKernelInfo>) -> Result<(), MapToError<Size4KiB>> {
    println!("[INFO][ALLOC] Locking alloc_inf");
    let mut lock = mutable_inf.lock();
    let locked = lock.deref_mut();
    
    println!("[INFO][ALLOC] Getting heap_start");
    let heap_start = locked.virt_region_allocator
        .alloc_pages(HEAP_PAGES as usize)
        .expect("Heap region");
    // let heap_sheap_starttart = VirtAddr::new(HEAP_START);
    
    println!("[INFO][ALLOC] Getting page range");
    let page_range = {
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    println!("[INFO][ALLOC] Mapping pages");
    for page in page_range {
        let frame = locked.frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { locked.page_table.map_to(page, frame, flags, &mut locked.frame_allocator)?.flush() };
    }
    drop(lock);

    let span = Span::from_base_size(heap_start.as_mut_ptr(), HEAP_SIZE as usize);

    let mut lock = ALLOCATOR.lock();

    println!("[INFO][ALLOC] Claiming heap");
    unsafe {
        lock.claim(span).expect("Claim heap");
    }

    println!("[INFO][ALLOC] set mutable_inf");
    lock.oom_handler.mutable_inf = Some(mutable_inf);

    drop(lock);

    Ok(())
}

#[global_allocator]
static ALLOCATOR: Talck<Mutex<()>, OomGrow> = Talc::new(OomGrow {mutable_inf: None}).lock();


struct OomGrow {
    mutable_inf: Option<&'static ReentrantMutex<AllocKernelInfo>>
}

const GROW_PAGES: u64 = 1024;

impl OomHandler for OomGrow {
    fn handle_oom(talc: &mut Talc<Self>, _layout: core::alloc::Layout) -> Result<(), ()> {
        println!("[INFO] Growing the HEAP");

        let mut lock = talc.oom_handler.mutable_inf.ok_or(()).inspect_err(|_| println!("[ERR] No kernel to claim"))?.lock();
        let kinf = lock.deref_mut();

        // TODO take layout into account
        let heap_start = kinf.virt_region_allocator
        .alloc_pages(GROW_PAGES as usize)
        .expect("Heap region");
        // let heap_sheap_starttart = VirtAddr::new(HEAP_START);
        let page_range = {
            let heap_end = heap_start + (GROW_PAGES * Size4KiB::SIZE) - 1u64;
            let heap_start_page = Page::containing_address(heap_start);
            let heap_end_page = Page::containing_address(heap_end);
            Page::range_inclusive(heap_start_page, heap_end_page)
        };

        for page in page_range {
            let frame = kinf.frame_allocator
                .allocate_frame()
                .ok_or(())?;
            let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
            unsafe { kinf.page_table.map_to(page, frame, flags, &mut kinf.frame_allocator).map_err(|_|())?.flush() };
        }
        drop(lock);

        let span = Span::from_base_size(heap_start.as_mut_ptr(), HEAP_SIZE as usize);

        unsafe {
            talc.claim(span)?;
        }
        Ok(())
    }
}

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
