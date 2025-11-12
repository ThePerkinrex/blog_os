use alloc::boxed::Box;
use cglue::trait_group::c_void;
use core::mem::MaybeUninit;

#[repr(C)]
pub struct CMaybeOwnedIterator<'a, T> {
    iter: &'a mut c_void,
    func: extern "C" fn(&mut c_void, out: &mut MaybeUninit<T>) -> i32,
    drop_fn: unsafe extern "C" fn(&mut c_void),
}

const extern "C" fn nothing(_: &mut c_void) {}

extern "C" fn drop_box<T>(ptr: &mut c_void) {
    let ptr = (ptr as *mut _) as *mut T;
    let boxed = unsafe { Box::from_raw(ptr) };
    drop(boxed)
}

impl<T> Drop for CMaybeOwnedIterator<'_, T> {
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.iter) };
    }
}

impl<'a, I: Iterator<Item = T>, T> From<&'a mut I> for CMaybeOwnedIterator<'a, T> {
    fn from(iter: &'a mut I) -> Self {
        CMaybeOwnedIterator::new_mut_ref(iter)
    }
}

impl<'a, T> CMaybeOwnedIterator<'a, T> {
    pub fn new_mut_ref<I: Iterator<Item = T>>(iter: &'a mut I) -> Self {
        extern "C" fn func<I: Iterator<Item = T>, T>(
            iter: &mut I,
            out: &mut MaybeUninit<T>,
        ) -> i32 {
            match iter.next() {
                Some(e) => {
                    unsafe { out.as_mut_ptr().write(e) };
                    0
                }
                None => 1,
            }
        }

        // SAFETY: type erasure is safe here, because the values are encapsulated and always in
        // a pair.
        let iter = unsafe { (iter as *mut _ as *mut c_void).as_mut().unwrap() };
        let func = func::<I, T> as extern "C" fn(_, _) -> _;
        let func = unsafe { core::mem::transmute::<_, _>(func) };

        Self {
            iter,
            func,
            drop_fn: nothing,
        }
    }

    pub fn new_owned<I: Iterator<Item = T> + 'a>(iter: I) -> Self {
        extern "C" fn func<I: Iterator<Item = T>, T>(
            iter: &mut I,
            out: &mut MaybeUninit<T>,
        ) -> i32 {
            iter.next().map_or(1, |e| {
                unsafe { out.as_mut_ptr().write(e) };
                0
            })
        }

        let boxed = Box::new(iter);
        let ptr: &'a mut I = Box::leak(boxed);

        // SAFETY: type erasure is safe here, because the values are encapsulated and always in
        // a pair.
        let iter = unsafe { (ptr as *mut _ as *mut c_void).as_mut().unwrap() };
        let func = func::<I, T> as extern "C" fn(_, _) -> _;
        let func = unsafe { core::mem::transmute::<_, _>(func) };

        Self {
            iter,
            func,
            drop_fn: drop_box::<I>,
        }
    }
}

impl<'a, T> Iterator for CMaybeOwnedIterator<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let mut out = MaybeUninit::uninit();
        if (self.func)(self.iter, &mut out) == 0 {
            Some(unsafe { out.assume_init() })
        } else {
            None
        }
    }
}
