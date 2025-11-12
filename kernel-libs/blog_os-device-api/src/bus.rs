use cglue::{cglue_trait, cglue_trait_group};

#[repr(C)]
pub struct BusDeviceIdOpaque {
    _data: (),
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[cglue_trait]
pub trait BusDeviceIdData {
    fn bus(&self) -> &'static str;
    fn data(&self) -> &BusDeviceIdOpaque;
}

cglue_trait_group!(BusDeviceId, {BusDeviceIdData, Display}, {});
#[cglue_trait]
pub trait Bus {
    fn name(&self) -> &'static str;
    fn connected_devices(
        &self,
    ) -> cglue::iter::CIterator<'_, (BusDeviceIdRef<'_>, Option<BusDeviceDriverRef<'_>>)>;
    fn register_driver(&mut self, driver: BusDeviceDriverBox<'static>);
}

#[cglue_trait]
pub trait BusDeviceDriver {
    fn name(&self) -> &'static str;
    fn bus(&self) -> &'static str;
}
