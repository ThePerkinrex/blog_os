use api_utils::{
    cglue::{self, cglue_trait, cglue_trait_group},
    iter::CMaybeOwnedIterator,
};

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

#[repr(C)]
pub struct BusDeviceMetadataOpaque {
    _data: (),
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

#[cglue_trait]
pub trait BusDeviceMetadata {
    fn bus(&self) -> &'static str;
    fn data(&self) -> &BusDeviceMetadataOpaque;
}

cglue_trait_group!(BusDeviceMetadataGroup, {BusDeviceMetadata, Display}, {});

#[cglue_trait]
pub trait Bus {
    fn name(&self) -> &'static str;
    fn connected_devices(
        &self,
    ) -> CMaybeOwnedIterator<
        '_,
        (
            BusDeviceIdRef<'_>,
            BusDeviceMetadataGroupRef<'_>,
            Option<BusDeviceDriverRef<'_>>,
        ),
    >;
    fn register_driver(&mut self, driver: BusDeviceDriverBox<'static>);
}

#[cglue_trait]
pub trait BusDeviceDriver {
    fn name(&self) -> &'static str;
    fn bus(&self) -> &'static str;
}
