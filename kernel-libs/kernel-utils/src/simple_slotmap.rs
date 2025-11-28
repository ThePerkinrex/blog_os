use alloc::vec::Vec;

/// A simple slotmap that always assigns the lowest free index.
pub struct SimpleSlotmap<T> {
    data: Vec<Option<T>>,
}

impl<T> SimpleSlotmap<T> {
    /// Create a new empty slotmap.
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Insert a value into the lowest available slot.
    /// Returns the assigned index.
    pub fn insert(&mut self, value: T) -> usize {
        // Find first empty slot
        for (idx, slot) in self.data.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(value);
                return idx;
            }
        }

        // No empty slot, push new
        let idx = self.data.len();
        self.data.push(Some(value));
        idx
    }

    /// Remove the value at index. Returns the removed value, if any.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.data.len() {
            return None;
        }

        let removed = self.data[index].take();

        // Trim trailing Nones
        while matches!(self.data.last(), Some(None)) {
            self.data.pop();
        }

        removed
    }

    /// Get immutable reference to value.
    pub fn get(&self, index: usize) -> Option<&T> {
        self.data.get(index).and_then(|v| v.as_ref())
    }

    /// Get mutable reference to value.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.data.get_mut(index).and_then(|v| v.as_mut())
    }

    /// Returns true if index contains a value.
    pub fn contains(&self, index: usize) -> bool {
        self.get(index).is_some()
    }

    /// Current number of occupied slots.
    pub fn len(&self) -> usize {
        self.data.iter().filter(|v| v.is_some()).count()
    }

    /// Current number of occupied slots.
    pub fn is_empty(&self) -> bool {
        self.data.iter().all(|v| v.is_none())
    }

    /// Capacity = highest index + 1 (including empty slots).
    pub const fn capacity(&self) -> usize {
        self.data.len()
    }

    /// Clears all entries.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Iterate over (index, &T)
    pub fn iter(&self) -> impl Iterator<Item = (usize, &T)> {
        self.data
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.as_ref().map(|v| (i, v)))
    }

    /// Iterate over (index, &mut T)
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut T)> {
        self.data
            .iter_mut()
            .enumerate()
            .filter_map(|(i, v)| v.as_mut().map(|v| (i, v)))
    }
}

impl<T> Default for SimpleSlotmap<T> {
    fn default() -> Self {
        Self::new()
    }
}
