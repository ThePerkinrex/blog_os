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

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;

    use crate::maybe_boxed::MaybeBoxed;

    #[test]
    fn deref_boxed() {
        let boxed = MaybeBoxed::<str, _>::Boxed(Box::from("Hello"));

        let deref = &*boxed;
        assert_eq!("Hello", deref);
    }

    #[test]
    fn deref_borrowed() {
        let borrowed = MaybeBoxed::<str, Box<str>>::Borrowed("Hello");

        let deref = &*borrowed;
        assert_eq!("Hello", deref);
    }
}
