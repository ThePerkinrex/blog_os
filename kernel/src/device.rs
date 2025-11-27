use blog_os_device::bus::BusRegistry;
use spin::{Lazy,RwLock};

pub static BUS_REGISTRY: Lazy<RwLock<BusRegistry>> = Lazy::new(|| RwLock::new(BusRegistry::default()));

