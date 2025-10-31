use core::{
    borrow::Borrow,
    cmp::Ordering,
    mem::MaybeUninit,
    ops::{Index, IndexMut},
};

use alloc::collections::btree_map::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct SmallBTreeMapEntry<K, V> {
    pub key: K,
    pub value: V,
}

#[derive(Debug)]
enum SmallBTreeMapInner<const N: usize, K, V> {
    Small {
        data: [MaybeUninit<SmallBTreeMapEntry<K, V>>; N],
        indices: [usize; N],
        len: usize,
    },
    Alloc(BTreeMap<K, V>),
}

#[derive(Debug)]
pub struct SmallBTreeMap<const N: usize, K, V>(SmallBTreeMapInner<N, K, V>);

impl<const N: usize, K, V> SmallBTreeMap<N, K, V>
where
    K: Ord,
{
    /// Binary search helper (returns Ok(index) if found, Err(insert_pos) if not)
    fn find_index<K2>(
        data: &[MaybeUninit<SmallBTreeMapEntry<K, V>>; N],
        indices: &[usize; N],
        len: usize,
        key: &K2,
    ) -> Result<usize, usize>
    where
        K: Borrow<K2>,
        K2: Ord + ?Sized,
    {
        let mut low = 0;
        let mut high = len;
        while low < high {
            let mid = (low + high) / 2;
            let idx = indices[mid];
            let entry = unsafe { data[idx].assume_init_ref() };
            match key.cmp(entry.key.borrow()) {
                Ordering::Less => high = mid,
                Ordering::Greater => low = mid + 1,
                Ordering::Equal => return Ok(mid),
            }
        }
        Err(low)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match &mut self.0 {
            SmallBTreeMapInner::Small { data, indices, len } => {
                // Binary search for position
                match Self::find_index(data, indices, *len, &key) {
                    Ok(i) => {
                        // Replace existing
                        let idx = indices[i];
                        let old = core::mem::replace(
                            &mut unsafe { data[idx].assume_init_mut() }.value,
                            value,
                        );
                        Some(old)
                    }
                    Err(pos) => {
                        // If full, promote
                        if *len == N {
                            let mut map = BTreeMap::new();
                            for x in data {
                                let e = unsafe { x.assume_init_read() };
                                map.insert(e.key, e.value);
                            }
                            map.insert(key, value);
                            self.0 = SmallBTreeMapInner::Alloc(map);
                            return None;
                        }

                        // Insert new entry
                        let slot = *len;
                        data[slot].write(SmallBTreeMapEntry { key, value });

                        // Shift indices to make room
                        indices.copy_within(pos..*len, pos + 1);
                        indices[pos] = slot;
                        *len += 1;
                        None
                    }
                }
            }
            SmallBTreeMapInner::Alloc(btree_map) => btree_map.insert(key, value),
        }
    }

    pub fn get<K2>(&self, key: &K2) -> Option<&V>
    where
        K: Borrow<K2>,
        K2: Ord + ?Sized,
    {
        match &self.0 {
            SmallBTreeMapInner::Small { data, indices, len } => {
                Self::find_index(data, indices, *len, key).map_or(None, |i| {
                    let idx = indices[i];
                    let entry = unsafe { data[idx].assume_init_ref() };
                    Some(&entry.value)
                })
            }
            SmallBTreeMapInner::Alloc(btree_map) => btree_map.get(key),
        }
    }

    pub fn get_mut<K2>(&mut self, key: &K2) -> Option<&mut V>
    where
        K: Borrow<K2>,
        K2: Ord + ?Sized,
    {
        match &mut self.0 {
            SmallBTreeMapInner::Small { data, indices, len } => {
                Self::find_index(data, indices, *len, key).map_or(None, |i| {
                    let idx = indices[i];
                    let entry = unsafe { data[idx].assume_init_mut() };
                    Some(&mut entry.value)
                })
            }
            SmallBTreeMapInner::Alloc(btree_map) => btree_map.get_mut(key),
        }
    }

    pub fn remove<K2>(&mut self, key: &K2) -> Option<V>
    where
        K: Borrow<K2>,
        K2: Ord + ?Sized,
    {
        match &mut self.0 {
            SmallBTreeMapInner::Small { data, indices, len } => {
                Self::find_index(data, indices, *len, key).map_or(None, |i| {
                    let idx = indices[i];
                    let entry = unsafe { data[idx].assume_init_read() };

                    // Shift indices left
                    indices.copy_within(i + 1..*len, i);
                    *len -= 1;
                    Some(entry.value)
                })
            }
            SmallBTreeMapInner::Alloc(btree_map) => btree_map.remove(key),
        }
    }

    pub fn contains_key<K2>(&self, key: &K2) -> bool
    where
        K: Borrow<K2>,
        K2: Ord + ?Sized,
    {
        match &self.0 {
            SmallBTreeMapInner::Small { data, indices, len } => {
                Self::find_index(data, indices, *len, key).is_ok()
            }
            SmallBTreeMapInner::Alloc(btree_map) => btree_map.contains_key(key),
        }
    }
}

impl<const N: usize, K, V> SmallBTreeMap<N, K, V> {
    pub const fn new() -> Self {
        Self(SmallBTreeMapInner::Small {
            data: [const { MaybeUninit::uninit() }; N],
            indices: [0; N],
            len: 0,
        })
    }

    pub fn len(&self) -> usize {
        match &self.0 {
            SmallBTreeMapInner::Small { len, .. } => *len,
            SmallBTreeMapInner::Alloc(map) => map.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> EntryIter<'_, K, V> {
        match &self.0 {
            SmallBTreeMapInner::Small { data, indices, len } => EntryIter::Small {
                entries: unsafe {
                    core::slice::from_raw_parts(
                        data.as_ptr() as *const SmallBTreeMapEntry<K, V>,
                        data.len(),
                    )
                },
                iter: indices.iter().take(*len),
            },
            SmallBTreeMapInner::Alloc(btree_map) => EntryIter::Alloc(btree_map.iter()),
        }
    }
}

impl<const N: usize, K, V> Default for SmallBTreeMap<N, K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, K, V> Index<&K> for SmallBTreeMap<N, K, V>
where
    K: Ord,
{
    type Output = V;

    fn index(&self, index: &K) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<const N: usize, K, V> IndexMut<&K> for SmallBTreeMap<N, K, V>
where
    K: Ord,
{
    fn index_mut(&mut self, index: &K) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}

pub enum EntryIter<'a, K, V> {
    Small {
        entries: &'a [SmallBTreeMapEntry<K, V>],
        iter: core::iter::Take<core::slice::Iter<'a, usize>>,
    },
    Alloc(alloc::collections::btree_map::Iter<'a, K, V>),
}

impl<'a, K, V> Iterator for EntryIter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Small { entries, iter } => iter
                .next()
                .and_then(|&i| entries.get(i))
                .map(|x| (&x.key, &x.value)),
            Self::Alloc(iter) => iter.next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;

    use super::SmallBTreeMap;

    #[test]
    fn basic_insert_and_get() {
        let mut map = SmallBTreeMap::<4, i32, &str>::new();

        assert!(map.is_empty());
        map.insert(2, "b");
        map.insert(1, "a");
        map.insert(3, "c");

        assert_eq!(map.len(), 3);
        assert_eq!(map.get(&1), Some(&"a"));
        assert_eq!(map.get(&2), Some(&"b"));
        assert_eq!(map.get(&3), Some(&"c"));
        assert_eq!(map.get(&4), None);
    }

    #[test]
    fn replace_existing_key() {
        let mut map = SmallBTreeMap::<4, i32, &str>::new();
        assert_eq!(map.insert(1, "a"), None);
        assert_eq!(map.insert(1, "new"), Some("a"));
        assert_eq!(map.get(&1), Some(&"new"));
    }

    #[test]
    fn remove_works() {
        let mut map = SmallBTreeMap::<4, i32, &str>::default();
        map.insert(1, "a");
        map.insert(2, "b");
        assert_eq!(map.remove(&1), Some("a"));
        assert_eq!(map.get(&1), None);
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn get_mut_works() {
        use alloc::string::String;
        let mut map = SmallBTreeMap::<4, i32, String>::new();
        map.insert(1, "a".into());
        let v = map.get_mut(&1).unwrap();
        v.push_str("bc");
        assert_eq!(map.get(&1).unwrap(), "abc");
    }

    #[test]
    fn remove_works_btreemap() {
        let mut map = SmallBTreeMap::<0, i32, &str>::new();
        map.insert(1, "a");
        map.insert(2, "b");
        assert_eq!(map.remove(&1), Some("a"));
        assert_eq!(map.get(&1), None);
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn get_mut_works_btreemap() {
        use alloc::string::String;
        let mut map = SmallBTreeMap::<0, i32, String>::new();
        map.insert(1, "a".into());
        let v = map.get_mut(&1).unwrap();
        v.push_str("bc");
        assert_eq!(map.get(&1).unwrap(), "abc");
    }

    #[test]
    fn promotion_to_btreemap() {
        let mut map = SmallBTreeMap::<3, i32, &str>::new();
        map.insert(1, "a");
        map.insert(2, "b");
        map.insert(3, "c");
        // This one triggers promotion
        map.insert(4, "d");

        // Should still work seamlessly
        assert_eq!(map.get(&4), Some(&"d"));
        assert_eq!(map.remove(&2), Some("b"));
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn use_btreemap() {
        let mut map = SmallBTreeMap::<3, i32, &str>::new();
        map.insert(1, "a");
        map.insert(2, "b");
        map.insert(3, "c");
        // This one triggers promotion
        map.insert(4, "d");
        map.insert(5, "e");

        // Should still work seamlessly
        assert_eq!(map.get(&4), Some(&"d"));
        assert_eq!(map.remove(&2), Some("b"));
        assert_eq!(map.len(), 4);
    }

    #[test]
    fn empty_map_behavior() {
        let map = SmallBTreeMap::<2, i32, i32>::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
        assert_eq!(map.get(&1), None);
        assert!(!map.contains_key(&1));
    }

    #[test]
    fn contains_key_works() {
        let mut map = SmallBTreeMap::<4, i32, &str>::new();
        map.insert(10, "ten");
        map.insert(20, "twenty");

        assert!(map.contains_key(&10));
        assert!(map.contains_key(&20));
        assert!(!map.contains_key(&30));
    }

    #[test]
    fn contains_key_works_btree_map() {
        let mut map = SmallBTreeMap::<0, i32, &str>::new();
        map.insert(10, "ten");
        map.insert(20, "twenty");

        assert!(map.contains_key(&10));
        assert!(map.contains_key(&20));
        assert!(!map.contains_key(&30));
    }

    #[test]
    fn index_and_index_mut_work() {
        let mut map = SmallBTreeMap::<4, i32, i32>::new();
        map.insert(1, 10);
        map.insert(2, 20);

        assert_eq!(map[&1], 10);
        assert_eq!(map[&2], 20);

        map[&1] = 99;
        assert_eq!(map[&1], 99);
    }

    #[test]
    fn iterates_in_sorted_order() {
        let mut map = SmallBTreeMap::<4, i32, &str>::new();
        map.insert(3, "c");
        map.insert(1, "a");
        map.insert(2, "b");

        let collected: alloc::vec::Vec<_> = map.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(collected, alloc::vec![(1, "a"), (2, "b"), (3, "c")]);
    }

    #[test]
    fn iterates_in_sorted_order_after_promotion() {
        let mut map = SmallBTreeMap::<2, i32, &str>::new();
        map.insert(10, "x");
        map.insert(5, "y");
        map.insert(15, "z"); // triggers promotion

        let collected: alloc::vec::Vec<_> = map.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(collected, alloc::vec![(5, "y"), (10, "x"), (15, "z")]);
    }

    #[test]
    fn promotion_to_btreemap_preserves_all_entries() {
        let mut map = SmallBTreeMap::<2, i32, &str>::new();
        map.insert(1, "a");
        map.insert(2, "b");
        map.insert(3, "c"); // triggers promotion

        assert_eq!(map.len(), 3);
        assert_eq!(map.get(&1), Some(&"a"));
        assert_eq!(map.get(&2), Some(&"b"));
        assert_eq!(map.get(&3), Some(&"c"));
    }

    #[test]
    fn promotion_does_not_break_existing_values() {
        let mut map = SmallBTreeMap::<2, i32, alloc::string::String>::new();
        map.insert(1, "one".into());
        map.insert(2, "two".into());
        map.insert(3, "three".into()); // triggers promotion

        let v = map.get(&2).unwrap();
        assert_eq!(v, "two");
    }

    #[test]
    fn overwriting_after_promotion_still_works() {
        let mut map = SmallBTreeMap::<2, i32, i32>::new();
        map.insert(1, 10);
        map.insert(2, 20);
        map.insert(3, 30); // triggers promotion
        assert_eq!(map.insert(2, 99), Some(20));
        assert_eq!(map.get(&2), Some(&99));
    }

    #[test]
    fn removing_nonexistent_key_returns_none() {
        let mut map = SmallBTreeMap::<4, i32, &str>::new();
        map.insert(1, "a");
        assert_eq!(map.remove(&2), None);
    }

    #[test]
    fn mixed_borrow_key_lookup() {
        use alloc::string::String;

        let mut map = SmallBTreeMap::<4, String, i32>::new();
        map.insert("hello".into(), 42);

        // Lookup using &str
        assert_eq!(map.get("hello"), Some(&42));
        assert!(map.contains_key("hello"));
        assert_eq!(map.remove("hello"), Some(42));
        assert!(map.get("hello").is_none());
    }

    #[test]
    fn iter_after_removal_reflects_changes() {
        let mut map = SmallBTreeMap::<4, i32, &str>::new();
        map.insert(1, "a");
        map.insert(2, "b");
        map.insert(3, "c");
        assert_eq!(map.len(), 3);

        map.remove(&2);
        assert_eq!(map.len(), 2);

        // assert_eq!(format!("{map:?}"), "");

        let collected: alloc::vec::Vec<_> = map.iter().map(|(k, v)| (*k, *v)).collect();
        assert_eq!(collected, alloc::vec![(1, "a"), (3, "c")]);
    }

    #[test]
    fn inserting_same_key_multiple_times_keeps_sorted_indices() {
        let mut map = SmallBTreeMap::<4, i32, i32>::new();
        map.insert(1, 10);
        map.insert(2, 20);

        map.insert(1, 100);
        map.insert(3, 30);

        let keys: alloc::vec::Vec<_> = map.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, alloc::vec![1, 2, 3]);
        assert_eq!(map.get(&1), Some(&100));
    }

    #[test]
    fn inserting_same_key_overwrites() {
        let mut map = SmallBTreeMap::<4, i32, i32>::new();
        map.insert(1, 10);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&1), Some(&10));
        assert_eq!(
            "SmallBTreeMap(Small { data: [MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>, MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>, MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>, MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>], indices: [0, 0, 0, 0], len: 1 })",
            format!("{map:?}")
        );
        map.insert(1, 20);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&1), Some(&20));
        assert_eq!(
            "SmallBTreeMap(Small { data: [MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>, MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>, MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>, MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>], indices: [0, 0, 0, 0], len: 1 })",
            format!("{map:?}")
        );

        map.insert(1, 100);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&1), Some(&100));
        assert_eq!(
            "SmallBTreeMap(Small { data: [MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>, MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>, MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>, MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>], indices: [0, 0, 0, 0], len: 1 })",
            format!("{map:?}")
        );
        map.insert(1, 30);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&1), Some(&30));
        assert_eq!(
            "SmallBTreeMap(Small { data: [MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>, MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>, MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>, MaybeUninit<kernel_utils::smallmap::SmallBTreeMapEntry<i32, i32>>], indices: [0, 0, 0, 0], len: 1 })",
            format!("{map:?}")
        );

        let keys: alloc::vec::Vec<_> = map.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, alloc::vec![1]);
        assert_eq!(map.get(&1), Some(&30));
    }

    #[test]
    fn btreemap_path_large_inserts() {
        let mut map = SmallBTreeMap::<1, i32, i32>::new();
        for i in 0..10 {
            map.insert(i, i * 10);
        }
        assert_eq!(map.len(), 10);
        for i in 0..10 {
            assert_eq!(map.get(&i), Some(&(i * 10)));
        }
    }

    #[test]
    #[should_panic]
    fn index_panics_on_missing_key() {
        let map = SmallBTreeMap::<2, i32, i32>::new();
        let _ = map[&1];
    }
}
