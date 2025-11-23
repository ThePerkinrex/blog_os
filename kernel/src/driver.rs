use core::{ffi::CStr, ops::Range};

use alloc::string::String;
use kdriver_api::{CLayout, KernelInterface};
use log::info;
use object::{Object, ObjectSymbol};
use thiserror::Error;
use x86_64::{
    VirtAddr,
    structures::paging::{PageSize, Size4KiB},
};

use crate::{
    elf::{EType, ElfLoadError, LoadedElf, load_elf, symbol::KDriverResolver},
    memory::range_alloc::FreeOnDrop,
    setup::KERNEL_INFO,
};

pub struct Interface {}

impl KernelInterface for Interface {
    fn abort(&self) {
        todo!("abort()")
    }

    fn print(&self, str: &str) {
        info!("print({str:?})")
    }
    unsafe fn alloc(&self, layout: CLayout) -> *mut u8 {
        todo!("alloc({layout:?})")
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: CLayout) {
        todo!("dealloc({ptr:p}, {layout:?})")
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
        let elf = load_elf(
            bytes,
            |e_type, size| {
                if *e_type == EType::ET_DYN {
                    let mut lock = KERNEL_INFO.get().unwrap().alloc_kinf.lock();

                    let pages = VirtAddr::new_truncate(size)
                        .align_up(Size4KiB::SIZE)
                        .as_u64()
                        / Size4KiB::SIZE;
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
            KDriverResolver::new(Interface {}),
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
}

impl Drop for KDriver {
    fn drop(&mut self) {
        info!("Stopping driver [{}:{}]", self.name, self.version);
        (self.stop)();
    }
}
