pub trait TryFromIterator<Item>: Sized {
    type Error;

    fn try_from_iter<T: IntoIterator<Item = Item>>(iter: T) -> Result<Self, Self::Error>;
}

pub trait FallibleCollectExt<Item> {
    fn try_collect<T>(self) -> Result<T, T::Error>
    where
        T: TryFromIterator<Item>;
}

impl<Item, I> FallibleCollectExt<Item> for I
where
    I: Iterator<Item = Item>,
{
    fn try_collect<T>(self) -> Result<T, T::Error>
    where
        T: TryFromIterator<Item>,
    {
        TryFromIterator::try_from_iter(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::try_from_iterator::{FallibleCollectExt, TryFromIterator};

    struct FailingTryCollect;

    impl<I> TryFromIterator<I> for FailingTryCollect {
        type Error = ();

        fn try_from_iter<T: IntoIterator<Item = I>>(_: T) -> Result<Self, Self::Error> {
            Err(())
        }
    }

    struct NotFailingTryCollect;

    impl<I> TryFromIterator<I> for NotFailingTryCollect {
        type Error = ();

        fn try_from_iter<T: IntoIterator<Item = I>>(_: T) -> Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    #[test]
    fn test_try_collect_fail() {
        let collected = core::iter::empty::<()>().try_collect::<FailingTryCollect>();
        assert!(collected.is_err())
    }

    #[test]
    fn test_try_collect() {
        let collected = core::iter::empty::<()>().try_collect::<NotFailingTryCollect>();
        assert!(collected.is_ok())
    }
}
