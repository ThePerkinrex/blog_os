use api_utils::{
    cglue::{self, cglue_trait, cglue_trait_group},
    iter::CMaybeOwnedIterator,
};

use crate::opaque_ptr;

#[cglue_trait]
pub trait AssociatedBusData<T> {
    fn bus(&self) -> &'static str;
    fn data(&self) -> &T;
}

opaque_ptr!(BusDeviceIdOpaque);

cglue_trait_group!(BusDeviceId, {AssociatedBusData<BusDeviceIdOpaque>, Display}, {});

opaque_ptr!(BusDeviceMetadataOpaque);

cglue_trait_group!(BusDeviceMetadata, {AssociatedBusData<BusDeviceMetadataOpaque>, Display}, {});

opaque_ptr!(BusDeviceInfoOpaque);

cglue_trait_group!(BusDeviceInfo, {AssociatedBusData<BusDeviceInfoOpaque>, Display}, {});

#[cglue_trait]
pub trait Bus {
    fn name(&self) -> &'static str;
    fn connected_devices(
        &self,
    ) -> CMaybeOwnedIterator<
        '_,
        (
            BusDeviceIdRef<'_>,
            BusDeviceMetadataRef<'_>,
            Option<BusDeviceDriverRef<'_>>,
        ),
    >;
    fn register_driver(&mut self, driver: BusDeviceDriverBox<'static>);
}

#[cglue_trait]
pub trait BusDeviceDriver {
    fn name(&self) -> &'static str;
    fn bus(&self) -> &'static str;
    fn matches(&self, metadata: BusDeviceMetadataRef<'_>) -> bool;
    fn register_device(
        &self,
        id: BusDeviceIdRef<'_>,
        metadata: BusDeviceMetadataRef<'_>,
        info: BusDeviceInfoRef<'_>,
    );
}
