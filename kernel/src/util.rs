use core::{
    mem::MaybeUninit,
    ops::{Deref, Index, IndexMut},
};

use alloc::{boxed::Box, sync::Arc};

#[derive(Debug)]
pub struct PtrOrdArc<T>(Arc<T>);

impl<T> From<Arc<T>> for PtrOrdArc<T> {
    fn from(value: Arc<T>) -> Self {
        Self(value)
    }
}

impl<T> Clone for PtrOrdArc<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Deref for PtrOrdArc<T> {
    type Target = Arc<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> PartialOrd for PtrOrdArc<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for PtrOrdArc<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        Arc::as_ptr(&self.0).cmp(&Arc::as_ptr(&other.0))
    }
}

impl<T> PartialEq for PtrOrdArc<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<T> Eq for PtrOrdArc<T> {}

pub struct NoHeapVec<const CAP: usize, T> {
    data: [MaybeUninit<T>; CAP],
    len: usize,
}

impl<const CAP: usize, T> From<[T; CAP]> for NoHeapVec<CAP, T> {
    fn from(value: [T; CAP]) -> Self {
        Self {
            data: value.map(MaybeUninit::new),
            len: CAP,
        }
    }
}

impl<const CAP: usize, T: core::fmt::Debug> core::fmt::Debug for NoHeapVec<CAP, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "[")?;
        for (i, v) in self.data.iter().take(self.len).enumerate() {
            write!(f, "{:?}", unsafe { v.assume_init_ref() })?;
            if i < (self.len - 1) {
                write!(f, ", ")?;
            }
        }
        write!(f, "]")
    }
}

impl<const CAP: usize, T> NoHeapVec<CAP, T> {
    pub fn new() -> Self {
        Self {
            data: core::array::from_fn(|_| MaybeUninit::uninit()),
            len: 0,
        }
    }

    pub fn push(&mut self, val: T) -> Result<(), &'static str> {
        if self.len == CAP {
            Err("Container full")
        } else {
            self.data[self.len] = MaybeUninit::new(val);
            self.len += 1;
            Ok(())
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            None
        } else {
            // Data under len is initialized
            Some(unsafe { self.data[index].assume_init_ref() })
        }
    }

    pub const fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            None
        } else {
            // Data under len is initialized
            Some(unsafe { self.data[index].assume_init_mut() })
        }
    }

    pub const fn first(&self) -> Option<&T> {
        self.get(0)
    }
}

impl<const CAP: usize, T> Default for NoHeapVec<CAP, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CAP: usize, T> Index<usize> for NoHeapVec<CAP, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).map_or_else(
            || panic!("Index out of range {index} >= {}", self.len),
            |x| x,
        )
    }
}

impl<const CAP: usize, T> IndexMut<usize> for NoHeapVec<CAP, T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let len = self.len;
        self.get_mut(index)
            .map_or_else(|| panic!("Index out of range {index} >= {}", len), |x| x)
    }
}

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
