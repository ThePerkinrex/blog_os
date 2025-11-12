use alloc::collections::btree_map::BTreeMap;
use blog_os_device_api::bus::{Bus, cglue_bus::BusBox};

#[derive(Default)]
pub struct BusRegistry {
    buses: BTreeMap<&'static str, BusBox<'static>>,
}

impl BusRegistry {
    pub fn register(&mut self, bus: BusBox<'static>) {
        self.buses.insert(bus.name(), bus);
    }

    pub fn unregister(&mut self, name: &str) -> Option<BusBox<'static>> {
        self.buses.remove(name)
    }
}
