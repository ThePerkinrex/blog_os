use core::ffi::CStr;

use alloc::collections::btree_map::BTreeMap;
use blog_os_device_api::bus::BusOps;

pub trait BusOpsSafe {
	fn name(&self) -> &'static CStr;
	fn free(self);
}

impl BusOpsSafe for BusOps {
	fn name(&self) -> &'static CStr {
		unsafe { self.name.as_ref() }.unwrap()
	}

	fn free(self) {
		drop(self)
	}
}

#[derive(Default)]
pub struct BusRegistry {
	buses: BTreeMap<&'static CStr, BusOps>
}


impl BusRegistry {
	pub fn register(&mut self, bus: BusOps) {
		self.buses.insert(bus.name(), bus);
	}

	pub fn unregister(&mut self, name: &CStr) -> Option<BusOps> {
		self.buses.remove(name)
	}
}