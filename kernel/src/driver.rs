use core::{alloc::Layout, ffi::CStr};

use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    string::{String, ToString},
    sync::Arc,
};
use blog_os_device::api::bus::{Bus, cglue_bus::BusBox};
use kdriver_api::{CLayout, KernelInterface};
use log::{debug, info};
use object::{Object, ObjectSymbol};
use spin::{Once, RwLock};
use thiserror::Error;
use x86_64::{
    VirtAddr,
    structures::paging::{PageSize, Size4KiB},
};

use crate::{
    device::BUS_REGISTRY,
    elf::{
        EType, ElfLoadError, LoadedElf, load_elf,
        symbol::{InterfaceKey, KDriverResolver},
    },
    memory::range_alloc::FreeOnDrop,
    setup::KERNEL_INFO,
};

pub mod registry;

struct InterfaceData {
    id: InterfaceKey,
    name: String,
    version: String,
}

pub struct Interface {
    data: Arc<Once<InterfaceData>>,
    allocs: Arc<RwLock<BTreeMap<VirtAddr, Layout>>>,
    registered_buses: Arc<RwLock<BTreeSet<String>>>,
}

impl Interface {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Once::new()),
            allocs: Arc::new(RwLock::new(BTreeMap::new())),
            registered_buses: Default::default(),
        }
    }
}

impl Default for Interface {
    fn default() -> Self {
        Self::new()
    }
}

impl KernelInterface for Interface {
    fn abort(&self) {
        todo!("abort()")
    }

    fn print(&self, str: &str) {
        let data = self.data.get().unwrap();
        info!(
            "[DRIVER][{}:{}/{:?}] {str}",
            data.name, data.version, data.id
        );
    }
    unsafe fn alloc(&self, layout: CLayout) -> *mut u8 {
        let layout = Layout::try_from(layout).unwrap();
        let ptr = unsafe { alloc::alloc::alloc(layout) };

        self.allocs.write().insert(VirtAddr::from_ptr(ptr), layout);

        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: CLayout) {
        let layout = Layout::try_from(layout).unwrap();
        if let Some(entry) = self.allocs.write().remove(&VirtAddr::from_ptr(ptr)) {
            if entry == layout {
                unsafe { alloc::alloc::dealloc(ptr, layout) };
            } else {
                panic!("freeing ptr with different layout than was allocated with")
            }
        } else {
            panic!("freeing ptr that was not allocated")
        }
    }

    fn register_bus(&self, bus: BusBox<'static>) {
        self.registered_buses.write().insert(bus.name().to_string());
        BUS_REGISTRY.write().register(bus);
    }
}

impl Drop for Interface {
    fn drop(&mut self) {
        for (&ptr, &layout) in self.allocs.read().iter() {
            unsafe { alloc::alloc::dealloc(ptr.as_mut_ptr::<u8>(), layout) };
        }

        for bus in self.registered_buses.read().iter() {
            BUS_REGISTRY.write().unregister(bus);
        }
    }
}

pub struct KDriver {
    _elf: LoadedElf<KDriverResolver>,
    name: String,
    version: String,
    start: extern "C" fn(),
    stop: extern "C" fn(),
    _region: Option<FreeOnDrop>,
}

impl core::fmt::Debug for KDriver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KDriver")
            .field("name", &self.name)
            .field("version", &self.version)
            .field("start", &self.start)
            .field("stop", &self.stop)
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum DriverLoadError {
    #[error(transparent)]
    Elf(#[from] ElfLoadError),
    #[error("Symbol is missing: {0:?}")]
    MissingSymbol(&'static str),
}

fn get_symbol<'file, 'data, O: Object<'data>>(
    object: &'file O,
    symbol_name: &'static str,
) -> Result<O::Symbol<'file>, DriverLoadError> {
    object
        .symbol_by_name(symbol_name)
        .ok_or(DriverLoadError::MissingSymbol(symbol_name))
}

impl KDriver {
    pub fn new(bytes: &[u8]) -> Result<Self, DriverLoadError> {
        let mut region = None;
        let interface = Interface::new();
        let data = interface.data.clone();
        let resolver = KDriverResolver::new(interface);
        let id = resolver.id();
        let elf = load_elf(
            bytes,
            |e_type, size| {
                if *e_type == EType::ET_DYN {
                    let mut lock = KERNEL_INFO.get().unwrap().alloc_kinf.lock();

                    let pages = size.div_ceil(Size4KiB::SIZE);
                    debug!("Requesting {pages} pages for driver (0x{size:X} bytes)");
                    let alloc_reg = lock
                        .virt_region_allocator
                        .allocate_range(pages)
                        .map_err(|_| ElfLoadError::MemAllocError)?;
                    let base_addr = VirtAddr::new_truncate(alloc_reg.start);
                    region = Some(alloc_reg);

                    drop(lock);

                    Ok(base_addr)
                } else {
                    Err(ElfLoadError::InvalidType(*e_type))
                }
            },
            false,
            resolver,
        )?;

        let base_addr = VirtAddr::new_truncate(elf.load_offset());

        let start_sym = get_symbol(elf.elf(), "start")?;
        let stop_sym = get_symbol(elf.elf(), "stop")?;
        let name_sym = get_symbol(elf.elf(), "NAME")?;
        let version_sym = get_symbol(elf.elf(), "VERSION")?;

        let start_addr = base_addr + start_sym.address();
        let stop_addr = base_addr + stop_sym.address();
        let name_addr = base_addr + name_sym.address();
        let version_addr = base_addr + version_sym.address();

        let start = unsafe {
            core::mem::transmute::<*const (), extern "C" fn()>(start_addr.as_ptr::<()>())
        };
        let stop =
            unsafe { core::mem::transmute::<*const (), extern "C" fn()>(stop_addr.as_ptr::<()>()) };
        let name = unsafe { CStr::from_ptr(name_addr.as_ptr::<*const core::ffi::c_char>().read()) }
            .to_string_lossy()
            .into_owned();
        let version =
            unsafe { CStr::from_ptr(version_addr.as_ptr::<*const core::ffi::c_char>().read()) }
                .to_string_lossy()
                .into_owned();

        data.call_once(|| InterfaceData {
            id,
            name: name.clone(),
            version: version.clone(),
        });

        Ok(Self {
            _elf: elf,
            name,
            version,
            start,
            stop,
            _region: region.map(FreeOnDrop),
        })
    }

    pub fn start(&self) {
        info!("Starting driver [{}:{}]", self.name, self.version);
        (self.start)()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}

impl Drop for KDriver {
    fn drop(&mut self) {
        info!("Stopping driver [{}:{}]", self.name, self.version);
        (self.stop)();
    }
}
