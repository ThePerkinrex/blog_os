use api_utils::cglue::{self, trait_obj};
use kdriver_api::cglue_kernelinterface::*;
use object::ObjectSymbol;
use x86_64::VirtAddr;

use crate::driver::Interface;

pub trait SymbolResolver {
    fn resolve<'data, 'file, S: ObjectSymbol<'data>>(&mut self, symbol: S) -> Option<VirtAddr>;
    fn unload(self);
}

impl SymbolResolver for () {
    fn resolve<'data, 'file, S: ObjectSymbol<'data>>(&mut self, _: S) -> Option<VirtAddr> {
        None
    }

    fn unload(self) {}
}

#[ouroboros::self_referencing]
struct KDriverInterface {
    interface: Interface,
    #[borrows(interface)]
    #[not_covariant]
    reference: KernelInterfaceRef<'this>,
}

impl KDriverInterface {
    pub fn create(interface: Interface) -> Self {
        Self::new(interface, |interface| {
            trait_obj!(interface as KernelInterface)
        })
    }
}

pub struct KDriverResolver {
    interface: KDriverInterface,
}

impl KDriverResolver {
    pub fn new(interface: Interface) -> Self {
        Self {
            interface: KDriverInterface::create(interface),
        }
    }
}

impl SymbolResolver for KDriverResolver {
    fn resolve<'data, 'file, S: ObjectSymbol<'data>>(&mut self, symbol: S) -> Option<VirtAddr> {
        if symbol.name() == Ok("INTERFACE") {
            let driver = &self.interface;

            let vaddr = driver.with_reference(|r| VirtAddr::from_ptr(r as *const _));

            Some(vaddr)
        } else {
            None
        }
    }

    fn unload(self) {
        drop(self.interface)
    }
}
