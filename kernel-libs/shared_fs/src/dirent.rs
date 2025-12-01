use core::{
    ffi::{CStr, c_char},
    ops::{Deref, DerefMut, Index, IndexMut},
};

use alloc::{borrow::Cow, string::String};

#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub struct DirEntryHeader {
    /// The size for the whole DirEntry struct, not just the name
    record_len: usize,
}

#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub struct DirEntry<T = [c_char]>
where
    T: ?Sized + Index<usize, Output = c_char> + IndexMut<usize>,
{
    header: DirEntryHeader,
    name: T,
}

impl<T> Deref for DirEntry<T>
where
    T: ?Sized + Index<usize, Output = c_char> + IndexMut<usize>,
{
    type Target = DirEntryHeader;

    fn deref(&self) -> &Self::Target {
        &self.header
    }
}

impl<T> DerefMut for DirEntry<T>
where
    T: ?Sized + Index<usize, Output = c_char> + IndexMut<usize>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.header
    }
}

impl DirEntry<[c_char]> {
    /// Constructs a `&mut DirEntry` from a thin pointer to its header.
    ///
    /// This function interprets a pointer to a `DirEntryHeader` as the start
    /// of a dynamically sized `DirEntry` struct, using the `record_len` field
    /// to determine the trailing slice length of the flexible array member
    /// `name`.
    ///
    /// # Safety
    ///
    /// Calling this function is **unsafe** because it relies on several
    /// invariants that the caller must uphold:
    ///
    /// - `thin` must be a valid, non-null pointer to the beginning of a
    ///   properly allocated `DirEntry` object in memory.
    /// - The memory region starting at `thin` must be at least
    ///   `(*thin).record_len` bytes long, covering both the header and the
    ///   trailing `name` array.
    /// - `record_len` must correctly describe the total size of the
    ///   `DirEntry` (header + name). If it is incorrect, creating the fat
    ///   pointer will result in undefined behavior.
    /// - The memory must be properly aligned for `DirEntry`.
    /// - The returned reference must not outlive the allocation backing
    ///   `thin`. Aliasing rules apply: you must not create overlapping
    ///   mutable references to the same memory.
    ///
    /// Violating any of these conditions can cause undefined behavior,
    /// including memory corruption, invalid reads/writes, or crashes.
    ///
    pub const unsafe fn from_thin<'a>(thin: *mut DirEntryHeader) -> &'a mut Self {
        let header: &DirEntryHeader = unsafe { thin.as_ref() }.unwrap();
        let size = header.record_len;

        let ptr = core::ptr::from_raw_parts_mut::<Self>(thin, size);

        unsafe { ptr.as_mut() }.unwrap()
    }
}

impl<T> DirEntry<T>
where
    T: ?Sized + Index<usize, Output = c_char> + IndexMut<usize>,
{
    /// Returns the `name` field as a UTF‑8 string.
    ///
    /// This method uses the `record_len` field to determine the maximum
    /// number of bytes available for the trailing `name` array, and then
    /// searches for the first NUL terminator within that bound. If no NUL
    /// is found, this function will use the whole field as a string.
    pub fn name(&self) -> Cow<'_, str> {
        // Compute the maximum number of bytes available for `name`.
        let header_size = core::mem::size_of::<DirEntryHeader>();
        assert!(self.header.record_len >= header_size);

        let name_bytes_len = self.header.record_len - header_size;

        // SAFETY: `self` is a valid DirEntry DST, so its data pointer
        // points to the header followed by `name`. We construct a slice
        // of the trailing bytes.
        let name_ptr = (self as *const Self) as *const u8;
        let name_ptr = unsafe { name_ptr.add(header_size) };

        let name_slice: &[u8] = unsafe { core::slice::from_raw_parts(name_ptr, name_bytes_len) };

        CStr::from_bytes_until_nul(name_slice).map_or_else(
            |_| String::from_utf8_lossy(name_slice),
            |cstr| cstr.to_string_lossy(),
        )
    }

    pub fn name_buf_mut(&mut self) -> &mut [u8] {
        let header_size = core::mem::size_of::<DirEntryHeader>();
        assert!(self.header.record_len >= header_size);

        let name_bytes_len = self.header.record_len - header_size;

        // SAFETY: `self` is a valid DirEntry DST, so its data pointer
        // points to the header followed by `name`. We construct a slice
        // of the trailing bytes.
        let name_ptr = (self as *mut Self) as *mut u8;
        let name_ptr = unsafe { name_ptr.add(header_size) };

        unsafe { core::slice::from_raw_parts_mut(name_ptr, name_bytes_len) }
    }
}

impl<const N: usize> DirEntry<[c_char; N]> {
    pub const fn new_const_cap() -> Self {
        let size = core::mem::size_of::<Self>();

        Self {
            header: DirEntryHeader { record_len: size },
            name: [0; N],
        }
    }
}
