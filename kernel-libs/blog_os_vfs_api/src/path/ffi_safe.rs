use alloc::string::String;
use alloc::vec;
use core::ffi::c_char;
use core::fmt;
use core::marker::PhantomData;
use core::ptr;
use core::str;

use crate::path::ContainsSlashError;
use crate::path::ffi;

// -----------------------------------------------------------------------------
// SafePathBuf (Owned)
// -----------------------------------------------------------------------------

/// An owned, safe wrapper around `PathBufOpaqueOwned`.
/// This struct manages the memory allocation and frees it when dropped.
#[repr(transparent)]
pub struct SafePathBuf {
    ptr: *mut ffi::PathBufOpaqueOwned,
}

// Safety: The underlying PathBuf is Send/Sync, and the FFI requires external synchronization
// for mutation, which Rust's ownership model guarantees.
unsafe impl Send for SafePathBuf {}
unsafe impl Sync for SafePathBuf {}

impl Drop for SafePathBuf {
    fn drop(&mut self) {
        // Safety: We exclusively own this pointer.
        unsafe { ffi::pathbuf_free(self.ptr) }
    }
}

impl SafePathBuf {
    /// Create a new empty `SafePathBuf`.
    pub fn new() -> Self {
        let ptr = unsafe { ffi::pathbuf_new() };
        assert!(!ptr.is_null(), "FFI returned null for pathbuf_new");
        Self { ptr }
    }

    /// Create a new root `SafePathBuf`.
    pub fn root() -> Self {
        let ptr = unsafe { ffi::pathbuf_root() };
        assert!(!ptr.is_null(), "FFI returned null for pathbuf_root");
        Self { ptr }
    }

    /// Parse a string. Returns `None` if the input is not valid UTF-8 (though standard Rust &str is always UTF-8)
    /// or if the internal FFI parser fails.
    pub fn parse(s: &str) -> Option<Self> {
        let ptr = unsafe { ffi::pathbuf_parse(s.as_ptr() as *const c_char, s.len()) };
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    /// Construct a SafePathBuf from a raw owned pointer.
    ///
    /// # Safety
    /// The caller must ensure `ptr` is a valid, owned pointer returned by the FFI constructors.
    /// This struct takes ownership and will call `pathbuf_free` on drop.
    pub unsafe fn from_raw(ptr: *mut ffi::PathBufOpaqueOwned) -> Self {
        debug_assert!(!ptr.is_null());
        Self { ptr }
    }

    /// Extract the raw pointer without dropping it.
    pub const fn into_raw(self) -> *mut ffi::PathBufOpaqueOwned {
        let ptr = self.ptr;
        core::mem::forget(self);
        ptr
    }

    /// Borrow as a `SafePathRef`.
    pub fn as_path_ref(&self) -> SafePathRef<'_> {
        unsafe { SafePathRef::from_ptr(self.ptr as *const ffi::PathBufOpaqueRef) }
    }

    /// Borrow as a `SafePathMut`.
    pub fn as_path_mut(&mut self) -> SafePathMut<'_> {
        unsafe { SafePathMut::from_ptr(self.ptr as *mut ffi::PathBufOpaqueMut) }
    }
}

impl Default for SafePathBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SafePathBuf {
    fn clone(&self) -> Self {
        self.as_path_ref().to_owned()
    }
}

impl fmt::Debug for SafePathBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.as_path_ref(), f)
    }
}

impl fmt::Display for SafePathBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.as_path_ref(), f)
    }
}

// -----------------------------------------------------------------------------
// SafePathRef (Borrowed, Immutable)
// -----------------------------------------------------------------------------

/// A borrowed, immutable view of a PathBuf.
/// Behaves like `&Path`.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct SafePathRef<'a> {
    ptr: *const ffi::PathBufOpaqueRef<'a>,
    _marker: PhantomData<&'a ()>,
}

unsafe impl<'a> Send for SafePathRef<'a> {}
unsafe impl<'a> Sync for SafePathRef<'a> {}

impl<'a> SafePathRef<'a> {
    /// Create from a raw const pointer.
    ///
    /// # Safety
    /// `ptr` must be valid for lifetime `'a`.
    pub unsafe fn from_ptr(ptr: *const ffi::PathBufOpaqueRef<'a>) -> Self {
        debug_assert!(!ptr.is_null());
        Self {
            ptr,
            _marker: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        unsafe { ffi::pathbuf_len(self.ptr) }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_absolute(&self) -> bool {
        unsafe { ffi::pathbuf_is_absolute(self.ptr) != 0 }
    }

    pub fn parent(&self) -> Option<SafePathBuf> {
        let ptr = unsafe { ffi::pathbuf_parent(self.ptr) };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { SafePathBuf::from_raw(ptr) })
        }
    }

    pub fn join(&self, other: SafePathRef<'_>) -> SafePathBuf {
        let ptr = unsafe { ffi::pathbuf_join(self.ptr, other.ptr) };
        assert!(!ptr.is_null(), "FFI join returned null");
        unsafe { SafePathBuf::from_raw(ptr) }
    }

    pub fn relative(&self, to: SafePathRef<'_>) -> Option<SafePathBuf> {
        let ptr = unsafe { ffi::pathbuf_relative(self.ptr, to.ptr) };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { SafePathBuf::from_raw(ptr) })
        }
    }

    pub fn to_owned(&self) -> SafePathBuf {
        // We simulate clone/to_owned by joining with an empty path,
        // as the FFI `join` creates a new allocation.
        // Alternatively, we could parse the string representation, but join is safer structurally.
        // However, we need a valid empty SafePathRef to join with.
        // Since we can't easily make a temporary empty one without allocating,
        // let's rely on the fact that `pathbuf_join` takes two refs.
        // Ideally the FFI would provide `pathbuf_clone`.
        // Hack: Use `pathbuf_relative` against the root? No.
        // Let's create a temporary empty path to join with.
        let empty = SafePathBuf::new();
        empty.as_path_ref().join(*self)
    }

    pub fn components(&self) -> Components<'a> {
        Components {
            path: *self,
            index: 0,
            count: unsafe { ffi::pathbuf_component_count(self.ptr) },
        }
    }
}

impl<'a> fmt::Display for SafePathRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            let req_len = ffi::pathbuf_display_len(self.ptr);
            if req_len == 0 {
                return Ok(());
            }

            // Allocate buffer + 1 for NUL terminator
            let mut buf = vec![0u8; req_len + 1];

            let written =
                ffi::pathbuf_write_display(self.ptr, buf.as_mut_ptr() as *mut c_char, buf.len());

            // Slicing up to `written` ensures we don't print the NUL byte
            let s = str::from_utf8(&buf[..written]).map_err(|_| fmt::Error)?;
            f.write_str(s)
        }
    }
}

impl<'a> fmt::Debug for SafePathRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// -----------------------------------------------------------------------------
// SafePathMut (Borrowed, Mutable)
// -----------------------------------------------------------------------------

/// A borrowed, mutable view of a PathBuf.
/// Behaves like `&mut PathBuf`.
#[repr(transparent)]
pub struct SafePathMut<'a> {
    ptr: *mut ffi::PathBufOpaqueMut<'a>,
    _marker: PhantomData<&'a mut ()>,
}

unsafe impl<'a> Send for SafePathMut<'a> {}
unsafe impl<'a> Sync for SafePathMut<'a> {}

impl<'a> SafePathMut<'a> {
    /// Create from a raw mut pointer.
    ///
    /// # Safety
    /// `ptr` must be valid for lifetime `'a` and have exclusive access.
    pub unsafe fn from_ptr(ptr: *mut ffi::PathBufOpaqueMut<'a>) -> Self {
        debug_assert!(!ptr.is_null());
        Self {
            ptr,
            _marker: PhantomData,
        }
    }

    /// downgrade to immutable ref
    pub fn as_path_ref(&self) -> SafePathRef<'_> {
        unsafe { SafePathRef::from_ptr(self.ptr as *const ffi::PathBufOpaqueRef) }
    }

    pub fn push_component(&mut self, component: &str) -> Result<(), ContainsSlashError> {
        let code = unsafe {
            ffi::pathbuf_push_component(
                self.ptr,
                component.as_ptr() as *const c_char,
                component.len(),
            )
        };

        match code {
            0 => Ok(()),
            1 => Err(ContainsSlashError),
            2 => panic!("FFI error: Invalid input provided to pathbuf_push_component"),
            _ => panic!("FFI error: Unknown error code {}", code),
        }
    }
}

impl<'a> fmt::Display for SafePathMut<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_path_ref().fmt(f)
    }
}

impl<'a> fmt::Debug for SafePathMut<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_path_ref().fmt(f)
    }
}

// -----------------------------------------------------------------------------
// Iterator
// -----------------------------------------------------------------------------

pub struct Components<'a> {
    path: SafePathRef<'a>,
    index: usize,
    count: usize,
}

impl<'a> Iterator for Components<'a> {
    type Item = String; // FFI forces a copy, so we yield owned String

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.count {
            return None;
        }

        unsafe {
            // 1. Get required size
            let req_len_isize =
                ffi::pathbuf_write_component(self.path.ptr, self.index, ptr::null_mut(), 0);

            if req_len_isize < 0 {
                return None;
            }

            let req_len = req_len_isize as usize;

            // 2. Allocate buffer (len + 1 for NUL)
            let mut buf = vec![0u8; req_len + 1];

            // 3. Write data
            let written_isize = ffi::pathbuf_write_component(
                self.path.ptr,
                self.index,
                buf.as_mut_ptr() as *mut c_char,
                buf.len(),
            );

            if written_isize < 0 {
                return None;
            }

            self.index += 1;

            // FFI returns bytes written excluding NUL.
            let s = String::from_utf8_lossy(&buf[..written_isize as usize]).into_owned();
            Some(s)
        }
    }
}

// -----------------------------------------------------------------------------
// Deref / Conversion
// -----------------------------------------------------------------------------

// Helper: Allow &SafePathBuf to be used where SafePathRef is expected
impl<'a> From<&'a SafePathBuf> for SafePathRef<'a> {
    fn from(owned: &'a SafePathBuf) -> Self {
        owned.as_path_ref()
    }
}

// Helper: Allow &mut SafePathBuf to be used where SafePathMut is expected
impl<'a> From<&'a mut SafePathBuf> for SafePathMut<'a> {
    fn from(owned: &'a mut SafePathBuf) -> Self {
        owned.as_path_mut()
    }
}
