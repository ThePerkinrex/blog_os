use alloc::{collections::btree_map::BTreeMap, string::String};
use spin::lock_api::RwLock;

use crate::driver::KDriver;

pub struct KDriverRegistry {
    drivers: BTreeMap<String, KDriver>,
}

impl KDriverRegistry {
    pub const fn new() -> Self {
        Self {
            drivers: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, driver: KDriver) -> Option<KDriver> {
        self.drivers.insert(driver.name.clone(), driver)
    }

    pub fn drivers(&self) -> impl Iterator<Item = &KDriver> {
        self.drivers.values()
    }

    pub fn remove(&mut self, driver_name: &str) -> Option<KDriver> {
        self.drivers.remove(driver_name)
    }
}

impl Default for KDriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub static DRIVER_REGISTRY: RwLock<KDriverRegistry> =
    RwLock::const_new(spin::RwLock::new(()), KDriverRegistry::new());
