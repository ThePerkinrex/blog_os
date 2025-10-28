use core::ops::Deref;

use alloc::boxed::Box;

pub enum MaybeBoxed<'a, T: ?Sized, B: Deref<Target = T> = Box<T>> {
    Borrowed(&'a T),
    Boxed(B),
}

impl<'a, T: ?Sized, B: Deref<Target = T>> Deref for MaybeBoxed<'a, T, B> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            MaybeBoxed::Borrowed(a) => a,
            MaybeBoxed::Boxed(b) => b,
        }
    }
}
