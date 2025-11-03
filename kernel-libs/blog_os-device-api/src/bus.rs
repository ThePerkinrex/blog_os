pub trait BusDriver {
    fn bus_name(&self) -> &'static str;
    fn notice_device(&mut self, name: &str);
}
