use core::{
    mem::MaybeUninit,
    ops::{Index, IndexMut},
};

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
        self.get(index)
            .unwrap_or_else(|| panic!("Index out of range {index} >= {}", self.len))
    }
}

impl<const CAP: usize, T> IndexMut<usize> for NoHeapVec<CAP, T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        let len = self.len;
        self.get_mut(index)
            .unwrap_or_else(|| panic!("Index out of range {index} >= {}", len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_empty() {
        let v: NoHeapVec<4, i32> = NoHeapVec::new();
        assert_eq!(v.len(), 0);
        assert!(v.is_empty());
    }

    #[test]
    fn push_and_len_work() {
        let mut v: NoHeapVec<3, i32> = NoHeapVec::new();
        assert_eq!(v.push(10), Ok(()));
        assert_eq!(v.len(), 1);
        assert!(!v.is_empty());
        assert_eq!(v.first(), Some(&10));

        v.push(20).unwrap();
        v.push(30).unwrap();
        assert_eq!(v.len(), 3);
    }

    #[test]
    fn push_returns_error_when_full() {
        let mut v: NoHeapVec<2, i32> = NoHeapVec::new();
        assert_eq!(v.push(1), Ok(()));
        assert_eq!(v.push(2), Ok(()));
        assert_eq!(v.push(3), Err("Container full"));
    }

    #[test]
    fn get_and_get_mut_work() {
        let mut v: NoHeapVec<3, i32> = NoHeapVec::new();
        v.push(5).unwrap();
        v.push(10).unwrap();

        assert_eq!(v.get(0), Some(&5));
        assert_eq!(v.get(1), Some(&10));
        assert_eq!(v.get(2), None);

        *v.get_mut(1).unwrap() = 99;
        assert_eq!(v.get(1), Some(&99));
    }

    #[test]
    fn index_and_index_mut_work() {
        let mut v: NoHeapVec<2, i32> = NoHeapVec::new();
        v.push(7).unwrap();
        v.push(8).unwrap();

        assert_eq!(v[0], 7);
        assert_eq!(v[1], 8);

        v[0] = 42;
        assert_eq!(v[0], 42);
    }

    #[test]
    #[should_panic(expected = "Index out of range 2 >= 2")]
    fn index_out_of_range_panics() {
        let mut v: NoHeapVec<2, i32> = NoHeapVec::new();
        v.push(1).unwrap();
        v.push(2).unwrap();
        let _ = v[2]; // should panic
    }

    #[test]
    #[should_panic(expected = "Index out of range 3 >= 2")]
    fn index_mut_out_of_range_panics() {
        let mut v: NoHeapVec<2, i32> = NoHeapVec::new();
        v.push(10).unwrap();
        v.push(20).unwrap();
        v[3] = 99; // should panic
    }

    #[test]
    fn from_array_works() {
        let v = NoHeapVec::from([1, 2, 3]);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0], 1);
        assert_eq!(v[1], 2);
        assert_eq!(v[2], 3);
    }

    #[test]
    fn debug_fmt_prints_correctly() {
        let v = NoHeapVec::from([1, 2, 3]);
        let s = alloc::format!("{:?}", v);
        assert_eq!(s, "[1, 2, 3]");
    }

    #[test]
    fn default_creates_empty() {
        let v: NoHeapVec<5, i32> = Default::default();
        assert_eq!(v.len(), 0);
        assert!(v.is_empty());
    }
}
