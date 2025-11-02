pub mod pci;

pub trait BusDriver {
    type For: Bus;

    fn notice_device(&mut self, name: &str);
}

pub trait Bus {
    const NAME: &str;

    fn devices(&self) -> impl Iterator<Item = &str>;
    fn register_driver<D: BusDriver<For = Self>>(&mut self, pattern: &str, driver: D);
}
