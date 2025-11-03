use alloc::{borrow::Cow, boxed::Box};
use blog_os_device::api::bus::BusDriver;

pub mod pci;

#[derive(Debug)]
pub struct PatternParseError;

pub trait Bus {
    fn name(&self) -> &'static str;
    fn devices(&self) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_>;
    fn register_driver(
        &mut self,
        pattern: &str,
        driver: Box<dyn BusDriver>,
    ) -> Result<(), PatternParseError>;
}
