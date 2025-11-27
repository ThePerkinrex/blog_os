#![no_std]

use core::alloc::{Layout, LayoutError};

use blog_os_device_api::bus::cglue_bus::BusBox;

#[derive(Debug)]
#[repr(C)]
pub struct CLayout {
    size: usize,
    align: usize,
}

impl From<Layout> for CLayout {
    fn from(value: Layout) -> Self {
        Self {
            size: value.size(),
            align: value.align(),
        }
    }
}

impl TryFrom<CLayout> for Layout {
    type Error = LayoutError;

    fn try_from(value: CLayout) -> Result<Self, Self::Error> {
        Self::from_size_align(value.size, value.align)
    }
}

#[cglue::cglue_trait]
pub trait KernelInterface {
    fn abort(&self);

    fn print(&self, str: &str);
    /// Allocates memory as described by the given `layout`.
    ///
    /// Returns a pointer to newly-allocated memory,
    /// or null to indicate allocation failure.
    ///
    /// # Safety
    ///
    /// `layout` must have non-zero size. Attempting to allocate for a zero-sized `layout` will
    /// result in undefined behavior.
    ///
    /// (Extension subtraits might provide more specific bounds on
    /// behavior, e.g., guarantee a sentinel address or a null pointer
    /// in response to a zero-size allocation request.)
    ///
    /// The allocated block of memory may or may not be initialized.
    ///
    /// # Errors
    ///
    /// Returning a null pointer indicates that either memory is exhausted
    /// or `layout` does not meet this allocator's size or alignment constraints.
    ///
    /// Implementations are encouraged to return null on memory
    /// exhaustion rather than aborting, but this is not
    /// a strict requirement. (Specifically: it is *legal* to
    /// implement this trait atop an underlying native allocation
    /// library that aborts on memory exhaustion.)
    ///
    /// Clients wishing to abort computation in response to an
    /// allocation error are encouraged to call the [`handle_alloc_error`] function,
    /// rather than directly invoking `panic!` or similar.
    ///
    unsafe fn alloc(&self, layout: CLayout) -> *mut u8;
    /// Deallocates the block of memory at the given `ptr` pointer with the given `layout`.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    ///
    /// * `ptr` is a block of memory currently allocated via this allocator and,
    ///
    /// * `layout` is the same layout that was used to allocate that block of
    ///   memory.
    ///
    /// Otherwise the behavior is undefined.
    unsafe fn dealloc(&self, ptr: *mut u8, layout: CLayout);


    fn register_bus(&self, bus: BusBox<'static>);

    
}
