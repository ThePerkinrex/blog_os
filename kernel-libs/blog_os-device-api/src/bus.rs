use cglue::cglue_trait;

#[cglue_trait]
pub trait Bus {
    fn name(&self) -> &'static str;
}

#[cglue_trait]
pub trait BusDeviceDriver {
    fn name(&self) -> &'static str;
    fn bus(&self) -> &'static str;
}
