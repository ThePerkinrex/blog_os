use crate::bus::Bus;

pub struct PciBus {}

impl Bus for PciBus {
    const NAME: &str = "pci";

    fn devices(&self) -> impl Iterator<Item = &str> {
        [""].iter().copied()
    }

    fn register_driver<D: super::BusDriver<For = Self>>(&mut self, pattern: &str, driver: D) {
        todo!()
    }
}
