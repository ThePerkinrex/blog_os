use core::{
    alloc::Layout,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::maybe_boxed::MaybeBoxed;

pub struct AlignedBytes {
    ptr: NonNull<u8>,
    len: usize,
    layout: Layout,
}

impl Drop for AlignedBytes {
    fn drop(&mut self) {
        unsafe {
            alloc::alloc::dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

impl AlignedBytes {
    pub fn new_aligned_copy<T>(og: &[u8]) -> Self {
        let size = og.len();
        let align = core::mem::align_of::<T>();
        let layout = Layout::from_size_align(size, align).unwrap();

        unsafe {
            let ptr = alloc::alloc::alloc(layout);
            let ptr = NonNull::new(ptr).expect("allocation failed");
            core::ptr::copy_nonoverlapping(og.as_ptr(), ptr.as_ptr(), size);

            Self {
                ptr,
                len: size,
                layout,
            }
        }
    }

    pub fn new_uninit<T>(size: usize) -> Self {
        let align = core::mem::align_of::<T>();
        let layout = Layout::from_size_align(size, align).unwrap();

        unsafe {
            let ptr = alloc::alloc::alloc(layout);
            let ptr = NonNull::new(ptr).expect("allocation failed");

            Self {
                ptr,
                len: size,
                layout,
            }
        }
    }

    pub const fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    pub const fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl Deref for AlignedBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for AlignedBytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_slice_mut()
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<&[u8]> for AlignedBytes {
    fn from(value: &[u8]) -> Self {
        let size = value.len();
        let layout = Layout::from_size_align(size, core::mem::align_of::<u8>()).unwrap();

        unsafe {
            let ptr = alloc::alloc::alloc(layout);
            let ptr = NonNull::new(ptr).expect("allocation failed");
            core::ptr::copy_nonoverlapping(value.as_ptr(), ptr.as_ptr(), size);
            Self {
                ptr,
                len: size,
                layout,
            }
        }
    }
}

unsafe impl Send for AlignedBytes {}
unsafe impl Sync for AlignedBytes {}

pub fn realign_if_necessary<'a, T>(og: &'a [u8]) -> MaybeBoxed<'a, [u8], AlignedBytes> {
    let align = core::mem::align_of::<T>();
    if (og.as_ptr() as usize).is_multiple_of(align) {
        MaybeBoxed::Borrowed(og)
    } else {
        MaybeBoxed::Boxed(AlignedBytes::new_aligned_copy::<T>(og))
    }
}
