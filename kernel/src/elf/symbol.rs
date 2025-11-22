use core::{alloc::Layout, borrow::Borrow, ops::Deref};

use alloc::{borrow::{Cow, ToOwned}, vec::Vec};
use api_utils::cglue::{self, trait_obj};
use kdriver_api::cglue_kernelinterface::*;
use object::ObjectSymbol;
use slotmap::{KeyData, SlotMap};
use spin::{Lazy, RwLock};

use crate::driver::Interface;

#[derive(Debug)]
pub struct SymbolData<'a> {
    pub data: Cow<'a, [u8]>,
}

impl<'a> SymbolData<'a> {
    pub fn new_borrowed<T>(data: &'a T) -> Self {
        let data = unsafe {
            core::slice::from_raw_parts(
                core::mem::transmute::<*const T, *const u8>(data as *const _),
                core::mem::size_of::<T>(),
            )
        };
        Self { data: Cow::Borrowed(data) }
    }
    pub fn new_owned<T>(data: &T) -> Self {
        let x = SymbolData::new_borrowed(data);
        Self {
            data: Cow::Owned(x.data.to_vec())
        }
    }
}

pub trait SymbolResolver {
    fn resolve<'data, 'file, S: ObjectSymbol<'data>>(&mut self, symbol: S) -> Option<SymbolData<'_>>;
}

impl SymbolResolver for () {
    fn resolve<'data, 'file, S: ObjectSymbol<'data>>(&mut self, _: S) -> Option<SymbolData<'_>> {
        None
    }
}

#[ouroboros::self_referencing]
struct KDriverInterface {
    interface: KernelInterfaceBox<'static>,
    #[borrows(interface)]
    reference: &'this KernelInterfaceBox<'static>,
}

impl KDriverInterface {
    pub fn create(interface: Interface) -> Self {
        Self::new(trait_obj!(interface as KernelInterface), |interface| {
            interface
        })
    }
}

slotmap::new_key_type! { struct InterfaceKey; }

static INTERFACES: Lazy<RwLock<SlotMap<InterfaceKey, KernelInterfaceBox<'static>>>> = Lazy::new(|| RwLock::new(SlotMap::with_key()));

extern "C" fn get_interface(id: u64) -> *const KernelInterfaceBox<'static> {
    let key = InterfaceKey::from(KeyData::from_ffi(id));

    INTERFACES.read().get(key).map(core::ptr::from_ref).unwrap_or(core::ptr::null())
}

pub struct KDriverResolver {
    id: InterfaceKey
}

impl KDriverResolver {
    pub fn new(interface: Interface) -> Self {
        let interface = trait_obj!(interface as KernelInterface);

        let key = INTERFACES.write().insert(interface); 

        Self { id: key }
    }
}

impl SymbolResolver for KDriverResolver {
    fn resolve<'data, 'file, S: ObjectSymbol<'data>>(&mut self, symbol: S) -> Option<SymbolData<'_>> {
        match symbol.name() {
            Ok("ID") => {
                let id = self.id.0.as_ffi();
                Some(SymbolData::new_owned(&id))
            },
            Ok("get_interface") => Some(SymbolData::new_borrowed(&get_interface)),
            _ => None
        }
    }
}
