use core::marker::PhantomData;

use alloc::boxed::Box;
use api_utils::cglue::{self, trait_obj};
use kdriver_api::cglue_kernelinterface::*;
use log::debug;
use object::ObjectSymbol;
use slotmap::{KeyData, SlotMap};
use spin::{Lazy, RwLock};
use x86_64::VirtAddr;

use crate::driver::Interface;

#[derive(Debug)]
pub struct SymbolData<'a> {
    pub data: VirtAddr,
    _marker: PhantomData<&'a ()>,
}

impl<'a> SymbolData<'a> {
    pub fn new_ref<T>(data: &'a T) -> Self {
        Self::new_ptr(data as *const T)
    }
    pub fn new_ptr<T>(data: *const T) -> Self {
        let data = VirtAddr::from_ptr(data);
        Self {
            data,
            _marker: PhantomData,
        }
    }
}

pub trait SymbolResolver {
    fn resolve<'data, 'file, S: ObjectSymbol<'data>>(
        &mut self,
        symbol: S,
    ) -> Option<SymbolData<'_>>;
}

impl SymbolResolver for () {
    fn resolve<'data, 'file, S: ObjectSymbol<'data>>(&mut self, _: S) -> Option<SymbolData<'_>> {
        None
    }
}

// #[ouroboros::self_referencing]
// struct KDriverInterface {
//     interface: KernelInterfaceBox<'static>,
//     #[borrows(interface)]
//     reference: &'this KernelInterfaceBox<'static>,
// }

// impl KDriverInterface {
//     pub fn create(interface: Interface) -> Self {
//         Self::new(trait_obj!(interface as KernelInterface), |interface| {
//             interface
//         })
//     }
// }

slotmap::new_key_type! { struct InterfaceKey; }

static INTERFACES: Lazy<RwLock<SlotMap<InterfaceKey, KernelInterfaceBox<'static>>>> =
    Lazy::new(|| RwLock::new(SlotMap::with_key()));

extern "C" fn get_interface(id: u64) -> *const KernelInterfaceBox<'static> {
    let key = InterfaceKey::from(KeyData::from_ffi(id));

    debug!("get_interface({id:x}) called (id: {key:?})");

    INTERFACES
        .read()
        .get(key)
        .map(core::ptr::from_ref)
        .unwrap_or(core::ptr::null())
}

pub struct KDriverResolver {
    id: InterfaceKey,
    ffi_key: Option<Box<u64>>,
}

impl KDriverResolver {
    pub fn new(interface: Interface) -> Self {
        let interface = trait_obj!(interface as KernelInterface);

        let key = INTERFACES.write().insert(interface);

        Self {
            id: key,
            ffi_key: None,
        }
    }
}

impl SymbolResolver for KDriverResolver {
    fn resolve<'data, 'file, 'a, S: ObjectSymbol<'data>>(
        &'a mut self,
        symbol: S,
    ) -> Option<SymbolData<'a>> {
        match symbol.name() {
            Ok("ID") => {
                let data: &'a u64 = &*self
                    .ffi_key
                    .get_or_insert_with(|| Box::new(self.id.0.as_ffi()));
                Some(SymbolData::new_ref(data))
            }
            Ok("get_interface") => Some(SymbolData::new_ptr(get_interface as *const ())),
            _ => None,
        }
    }
}

impl Drop for KDriverResolver {
    fn drop(&mut self) {
        INTERFACES.write().remove(self.id);
    }
}
