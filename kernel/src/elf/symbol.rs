use core::alloc::Layout;

use api_utils::cglue::{self, trait_obj};
use kdriver_api::cglue_kernelinterface::*;
use object::ObjectSymbol;

use crate::driver::Interface;

#[derive(Debug)]
pub struct SymbolData<'a> {
    layout: Layout,
    pub data: &'a [u8],
}

impl<'a> SymbolData<'a> {
    pub fn new<T>(data: &'a T) -> Self {
        let layout = Layout::new::<T>();
        let data = unsafe {
            core::slice::from_raw_parts(
                core::mem::transmute::<*const T, *const u8>(data as *const _),
                layout.size(),
            )
        };
        Self { layout, data }
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

pub struct KDriverResolver {
    interface: KDriverInterface
}

impl KDriverResolver {
    pub fn new(interface: Interface) -> Self {
        Self {
            interface: KDriverInterface::create(interface),
        }
    }
}

impl SymbolResolver for KDriverResolver {
    fn resolve<'data, 'file, S: ObjectSymbol<'data>>(&mut self, symbol: S) -> Option<SymbolData<'_>> {
        if symbol.name() == Ok("INTERFACE") {
            let driver = &self.interface;

            let data = SymbolData::new::<&KernelInterfaceBox<'static>>(driver.borrow_reference());

            Some(data)
        } else {
            None
        }
    }
}
