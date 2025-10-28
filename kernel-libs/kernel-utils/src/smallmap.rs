use core::{
    borrow::Borrow,
    cmp::Ordering,
    mem::MaybeUninit,
    ops::{Deref, Index, IndexMut},
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
                            unsafe { &mut (*data[idx].as_mut_ptr()).value },
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
                        *len,
                    )
                },
                iter: indices.iter(),
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
        iter: core::slice::Iter<'a, usize>,
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
        let mut map = SmallBTreeMap::<4, i32, &str>::new();
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
        if let Some(v) = map.get_mut(&1) {
            v.push_str("bc");
        }
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
}
