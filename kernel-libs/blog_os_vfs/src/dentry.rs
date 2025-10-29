use core::num::NonZeroU64;

use kernel_utils::smallmap::SmallBTreeMap;

use crate::{
    INodeRef,
    path::{Path, PathBuf},
};

pub struct DEntry {
    pub inode: INodeRef,
}

pub enum DEntryStatus {
    MountPoint,
    LastAccess(NonZeroU64),
}

impl DEntryStatus {
    pub const fn set_version(&mut self, version: NonZeroU64) {
        match self {
            Self::MountPoint => (),
            Self::LastAccess(non_zero) => *non_zero = version,
        }
    }
}

pub struct DEntryCache {
    map: SmallBTreeMap<1, PathBuf, (DEntry, DEntryStatus)>,
    version: NonZeroU64, // TODO implement cleaning
}

impl DEntryCache {
    pub const fn new() -> Self {
        Self {
            map: SmallBTreeMap::new(),
            version: unsafe { NonZeroU64::new_unchecked(1) },
        }
    }

    pub fn get_mut(&mut self, key: &Path) -> Option<&mut DEntry> {
        self.map.get_mut(key).map(|(x, status)| {
            status.set_version(self.version);
            self.version = self.version.saturating_add(1);
            x
        })
    }

    pub fn add_mountpoint(&mut self, key: PathBuf, entry: DEntry) {
        self.map.insert(key, (entry, DEntryStatus::MountPoint));
    }

    pub fn add_cached(&mut self, key: PathBuf, entry: DEntry) {
        self.map
            .insert(key, (entry, DEntryStatus::LastAccess(self.version)));
        self.version = self.version.saturating_add(1);
    }

    pub fn remove(&mut self, key: &Path) -> Option<DEntry> {
        self.map.remove(key).map(|(x, _)| x)
    }

    pub fn find_greatest<'a, 'b>(
        &'a mut self,
        mut key: &'b Path,
    ) -> Option<(&'a mut DEntry, &'b Path)> {
        let key = loop {
            if self.map.contains_key(key) {
                break Some(key);
            }

            if let Some(parent) = key.parent() {
                key = parent
            } else {
                break None;
            }
        };

        key.and_then(|x| self.get_mut(x).map(move |a| (a, x)))
    }
}

impl Default for DEntryCache {
    fn default() -> Self {
        Self::new()
    }
}
