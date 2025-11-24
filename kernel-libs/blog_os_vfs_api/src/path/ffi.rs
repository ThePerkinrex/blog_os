use core::{ffi::c_char, marker::PhantomData, ptr, slice};

use alloc::{borrow::ToOwned, boxed::Box};

use crate::path::{ContainsSlashError, PathBuf};

#[repr(transparent)]
pub struct PathBufOpaqueOwned(PathBuf);

#[repr(transparent)]
pub struct PathBufOpaqueRef<'a>(PathBuf, PhantomData<&'a ()>);

#[repr(transparent)]
pub struct PathBufOpaqueMut<'a>(PathBuf, PhantomData<&'a mut ()>);

#[inline(always)]
fn wrap(pb: PathBuf) -> *mut PathBufOpaqueOwned {
    Box::into_raw(Box::new(PathBufOpaqueOwned(pb)))
}

// ------------------------
// Constructors
// ------------------------

/// Create a new empty `PathBuf`.
///
/// # Safety
///
/// - This function is `unsafe` because it returns a raw pointer into heap memory.
/// - The returned pointer is non-null and points to a heap allocation that contains
///   a `PathBufOpaque`. The caller takes ownership of this pointer and **must**
///   eventually release it by calling `pathbuf_free` exactly once.
/// - Do **not** call `pathbuf_free` twice on the same pointer (no double-free).
/// - Do **not** attempt to read from or write to the returned pointer from other
///   threads concurrently unless you synchronize externally. The `PathBufOpaque`
///   type is not synchronized by this API.
///
/// Returns: owned pointer to a newly allocated `PathBufOpaque`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_new() -> *mut PathBufOpaqueOwned {
    wrap(PathBuf::new())
}

/// Create a new `PathBuf` representing the root.
///
/// # Safety
///
/// - Same invariants as `pathbuf_new`.
/// - Caller owns the returned pointer and must call `pathbuf_free` to deallocate it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_root() -> *mut PathBufOpaqueOwned {
    wrap(PathBuf::root())
}

/// Free a `PathBufOpaque` previously returned by this API.
///
/// # Safety
///
/// - `ptr` must either be a pointer previously returned by one of this module's
///   constructor functions (for example `pathbuf_new`, `pathbuf_root`,
///   `pathbuf_parse`, `pathbuf_parent`, `pathbuf_join`, `pathbuf_relative`), or
///   a null pointer.
/// - If `ptr` is non-null it must be the exclusive owner of that allocation.
///   Calling `pathbuf_free` with a non-owned pointer, a pointer that has been
///   freed already, or a pointer not produced by these constructors is undefined
///   behavior (UB).
/// - It is safe to call `pathbuf_free(ptr)` with `ptr == NULL` (a no-op).
/// - The caller must ensure there are no outstanding borrows or concurrent
///   accesses to the pointed-to `PathBufOpaque` while freeing it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_free(ptr: *mut PathBufOpaqueOwned) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(ptr) });
    }
}

// ------------------------
// Parsing
// ------------------------

/// Parse a UTF-8 byte sequence of length `len` at `ptr` as a path and return a
/// newly allocated `PathBufOpaque` on success, or NULL on failure.
///
/// Returns a new heap-allocated `PathBufOpaque` (ownership transferred to the
/// caller) or NULL if parsing fails (invalid UTF-8).
///
/// # Safety
///
/// - `ptr` must point to `len` contiguous bytes that are readable by the current
///   thread for the duration of the call.
/// - If `len != 0`, `ptr` **must not** be NULL. If `len == 0` then `ptr` may be
///   NULL and the empty string will be parsed.
/// - The bytes referenced by `ptr` must be a valid UTF-8 sequence. If they are
///   not valid UTF-8 this function will return `NULL`.
/// - The returned pointer, if non-NULL, is owned by the caller and must be
///   freed with `pathbuf_free`.
/// - There is no synchronization: callers must ensure exclusive access if the
///   same underlying memory is accessed concurrently.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_parse(ptr: *const c_char, len: usize) -> *mut PathBufOpaqueOwned {
    if ptr.is_null() && len != 0 {
        return ptr::null_mut();
    }
    let bytes = unsafe { slice::from_raw_parts(ptr as *const u8, len) };
    let s = match str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    wrap(PathBuf::parse(s))
}

// ------------------------
// Push component
// ------------------------

/// Push a path component (UTF-8 string) onto `pb`.
///
/// Return codes:
/// - `0` success
/// - `1` component contained a slash (`ContainsSlashError`)
/// - `2` invalid input (null pointer, invalid utf-8, or null `pb`)
///
/// # Safety
///
/// - `pb` must be a non-null pointer previously returned by this crate (for
///   example `pathbuf_new` / `pathbuf_parse` / ...). Passing a pointer not
///   produced by this API, or a pointer that has already been freed, is UB.
/// - The caller must ensure exclusive / mutable access to `pb` for the duration
///   of the call. Concurrent mutation or concurrent use from other threads is
///   undefined unless the caller provides synchronization.
/// - `ptr` must point to `len` readable bytes. If `len != 0`, `ptr` must not be
///   NULL. If the bytes are not valid UTF-8 this function returns `2`.
/// - The function takes ownership of the string data by cloning/boxing a Rust
///   `String` internally; the caller retains no ownership of `ptr`.
/// - This function will not panic across the FFI boundary based on ordinary
///   invalid input (it signals failure via return codes) but any misuse that
///   violates the above preconditions may cause UB.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_push_component(
    pb: *mut PathBufOpaqueMut,
    ptr: *const c_char,
    len: usize,
) -> i32 {
    if pb.is_null() {
        return 2;
    }
    let bytes = unsafe { slice::from_raw_parts(ptr as *const u8, len) };
    let s = match str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return 2,
    };
    match unsafe { &mut *pb }.0.push_component(Box::from(s)) {
        Ok(()) => 0,
        Err(ContainsSlashError) => 1,
    }
}

// ------------------------
// Simple “view” accessors
// ------------------------

/// Return the number of components in the path. If `pb` is NULL returns 0.
///
/// # Safety
///
/// - `pb` may be NULL (this function returns 0 in that case).
/// - If `pb` is non-null it must be a valid pointer returned by this crate and
///   must remain valid for the duration of the call.
/// - This is a read-only accessor; the caller must ensure the pointed value is
///   not being concurrently mutated unsafely.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_len(pb: *const PathBufOpaqueRef) -> usize {
    if pb.is_null() {
        return 0;
    }
    unsafe { &*pb }.0.len()
}

/// Return `1` if the path is absolute, otherwise `0`. If `pb` is NULL returns `0`.
///
/// # Safety
///
/// - Same preconditions as `pathbuf_len`: `pb` may be NULL, otherwise must be a
///   valid pointer owned by the caller.
/// - The value is computed immutably; ensure no concurrent mutable alias exists.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_is_absolute(pb: *const PathBufOpaqueRef) -> u8 {
    if pb.is_null() {
        return 0;
    }
    unsafe { &*pb }.0.is_absolute() as u8
}

// ------------------------
// parent()
// ------------------------

/// Return a newly-allocated `PathBufOpaque` representing the parent of `pb`,
/// or NULL if `pb` is NULL or has no parent.
///
/// The returned pointer, if non-null, must be freed with `pathbuf_free`.
///
/// # Safety
///
/// - `pb` may be NULL, in which case the function returns `NULL`.
/// - If `pb` is non-null it must be a valid pointer previously returned by
///   this crate and must remain valid for the duration of the call.
/// - The returned `PathBufOpaque` is newly allocated (ownership transferred to
///   the caller). The caller must call `pathbuf_free` on the returned pointer
///   when finished.
/// - The caller must ensure there are no concurrent unsynchronized
///   mutations/borrows of `pb` while calling this function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_parent(pb: *const PathBufOpaqueRef) -> *mut PathBufOpaqueOwned {
    if pb.is_null() {
        return ptr::null_mut();
    }
    unsafe { &*pb }
        .0
        .parent()
        .map_or(ptr::null_mut(), |p| wrap(p.to_owned()))
}

// ------------------------
// join()
// ------------------------

/// Return a newly-allocated `PathBufOpaque` that is `left.join(right)`, or NULL
/// if either input is NULL.
///
/// The result must be freed with `pathbuf_free`.
///
/// # Safety
///
/// - `left` and `right` must either be both non-null and valid pointers returned
///   by this crate, or the function will return `NULL`.
/// - If non-null, both pointers must remain valid for the duration of the call.
/// - The returned pointer is owned by the caller and must be freed with
///   `pathbuf_free`.
/// - This function performs immutable reads of `left` and `right`; callers must
///   ensure no concurrent unsynchronized mutations occur during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_join(
    left: *const PathBufOpaqueRef,
    right: *const PathBufOpaqueRef,
) -> *mut PathBufOpaqueOwned {
    if left.is_null() || right.is_null() {
        return ptr::null_mut();
    }
    wrap(unsafe { &*left }.0.join(&(unsafe { &*right }).0))
}

// ------------------------
// relative()
// ------------------------

/// Compute `a.relative(b)` and return a newly-allocated `PathBufOpaque` or NULL
/// on error (or if `a` or `b` is NULL).
///
/// # Safety
///
/// - `a` and `b` must be valid, non-null pointers produced by this crate (or
///   the function will return NULL).
/// - The returned pointer, if non-null, is owned by the caller and must be
///   freed with `pathbuf_free`.
/// - Both input pointers must remain valid for the duration of the call and not
///   be mutated concurrently without synchronization.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_relative(
    a: *const PathBufOpaqueRef,
    b: *const PathBufOpaqueRef,
) -> *mut PathBufOpaqueOwned {
    if a.is_null() || b.is_null() {
        return ptr::null_mut();
    }
    (unsafe { &*a })
        .0
        .relative(&(unsafe { &*b }).0)
        .map_or(ptr::null_mut(), |rel| wrap(rel.to_owned()))
}

// ------------------------
// Display helpers
// ------------------------

/// Returns required buffer length (no NUL terminator included).
///
/// # Safety
///
/// - `pb` may be NULL; in that case the function returns 0.
/// - If `pb` is non-null, it must be a valid pointer previously returned by
///   this crate and must remain valid for the duration of the call.
/// - This function allocates a temporary `String` internally to compute the
///   length; callers should not rely on pointer stability across threads.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_display_len(pb: *const PathBufOpaqueRef) -> usize {
    if pb.is_null() {
        return 0;
    }
    let s = alloc::format!("{}", (unsafe { &*pb }).0);
    s.len()
}

/// Writes the display string into `out` up to `out_len - 1` bytes and NUL-terminates.
///
/// Returns number of bytes written (not counting the trailing NUL). If `out`
/// is NULL or `out_len == 0` the function returns the number of bytes that
/// would have been written (the required length).
///
/// # Safety
///
/// - If `pb` is non-null it must be a valid pointer previously returned by
///   this crate and remain valid for the duration of the call.
/// - If `out_len != 0`, then `out` must be a valid pointer to `out_len` bytes
///   of writable memory. If `out_len == 0`, `out` may be NULL (the function
///   will simply return the required length).
/// - The function writes at most `out_len` bytes; when `out_len > 0` the
///   function will always write a NUL byte at `out + write_len` (so the caller
///   must provide space for that NUL). The function writes `min(s.len(), out_len-1)`
///   bytes followed by a single 0 byte.
/// - The caller must not rely on any particular encoding of the returned bytes
///   beyond UTF-8 (the display string is UTF-8), and must ensure the buffer is
///   properly freed/managed on the C side.
/// - Concurrent mutable access to `out` from other threads while this function
///   runs is undefined behavior; ensure exclusive access.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_write_display(
    pb: *const PathBufOpaqueRef,
    out: *mut c_char,
    out_len: usize,
) -> usize {
    if pb.is_null() {
        return 0;
    }

    let s = alloc::format!("{}", (unsafe { &*pb }).0);
    let bytes = s.as_bytes();

    if out.is_null() || out_len == 0 {
        return bytes.len();
    }

    let write_len = core::cmp::min(bytes.len(), out_len - 1);
    let out_slice = unsafe { slice::from_raw_parts_mut(out as *mut u8, out_len) };
    out_slice[..write_len].copy_from_slice(&bytes[..write_len]);
    out_slice[write_len] = 0;

    write_len
}

// ------------------------
// Component access
// ------------------------

/// Return the number of components in the path. If `pb` is NULL returns 0.
///
/// # Safety
///
/// - `pb` may be NULL (this function returns 0 in that case).
/// - If `pb` is non-null it must be a valid pointer returned by this crate and
///   must remain valid for the duration of the call.
/// - This is a read-only accessor; the caller must ensure the pointed value is
///   not being concurrently mutated unsafely.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_component_count(pb: *const PathBufOpaqueRef) -> usize {
    unsafe { pathbuf_len(pb) }
}

/// Write the `index`-th path component (UTF-8 bytes) into `out`. Return values:
/// - `>= 0` number of bytes written (not counting final NUL)
/// - `-1` `pb` was NULL
/// - `-2` index out of range
///
/// If `out` is NULL or `out_len == 0` the function returns the number of bytes
/// that would be required to store the component (not counting a trailing NUL).
///
/// # Safety
///
/// - `pb` must be NULL or a valid pointer previously returned by this crate.
///   If `pb` is NULL this function returns `-1`.
/// - If `index` is out of range this function returns `-2`.
/// - If `out_len != 0`, `out` must point to `out_len` writable bytes. The
///   function will write at most `out_len` bytes and will write a terminating
///   NUL at `out + write_len` when `out_len > 0`. The caller must ensure the
///   buffer is large enough for the data they want to receive (call with
///   `out == NULL`/`out_len == 0` to probe required size).
/// - The returned component bytes are UTF-8. The caller must treat them as such.
/// - The pointer returned/used by this function does not transfer ownership:
///   only the `PathBufOpaque` returned by constructors is owned by the caller.
/// - Avoid concurrent unsynchronized mutations of `pb` while calling this
///   function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pathbuf_write_component(
    pb: *const PathBufOpaqueRef,
    index: usize,
    out: *mut c_char,
    out_len: usize,
) -> isize {
    if pb.is_null() {
        return -1;
    }
    let p = (unsafe { &*pb }).0.as_path();

    let comp = match p.components().nth(index) {
        Some(s) => s,
        None => return -2, // index out of range
    };

    let bytes = comp.as_bytes();

    if out.is_null() || out_len == 0 {
        return bytes.len() as isize;
    }

    let write_len = core::cmp::min(bytes.len(), out_len - 1);
    let out_slice = unsafe { slice::from_raw_parts_mut(out as *mut u8, out_len) };
    out_slice[..write_len].copy_from_slice(&bytes[..write_len]);
    out_slice[write_len] = 0;

    write_len as isize
}
