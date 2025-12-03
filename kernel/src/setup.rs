use addr2line::Context;
use alloc::vec::Vec;
use bootloader_api::{
    config::ApiVersion,
    info::{MemoryRegionKind, TlsTemplate},
};
use log::{debug, info, trace, warn};
use qemu_common::KERNEL_START;
use spin::Once;
use x86_64::{
    VirtAddr,
    structures::paging::{FrameDeallocator, Mapper, Size4KiB},
};

use crate::{
    allocator,
    // bus::{Bus, pci::PciBus},
    dwarf::{EndianSlice, load_dwarf},
    elf::SystemElf,
    gdt,
    interrupts,
    io,
    memory::{
        self, BootInfoFrameAllocator, multi_l4_paging::PageTables, range_alloc::RangeAllocator,
    },
    multitask::{self, lock::ReentrantMutex},
    stack::{self, SlabStack, StackAlloc},
    unwind::eh::EhInfo,
};

pub type KernelElfFile = SystemElf<'static>;

pub struct AllocKernelInfo {
    /// The kernel page tables
    pub page_table: PageTables,
    pub frame_allocator: BootInfoFrameAllocator,
    pub virt_region_allocator: RangeAllocator<u64>,
}

impl AllocKernelInfo {
    pub fn create_p4_table_and_switch(
        &mut self,
    ) -> alloc::sync::Arc<memory::multi_l4_paging::PageTableToken> {
        self.page_table
            .create_process_p4_and_switch(&mut self.frame_allocator)
    }
}

pub struct KernelInfo {
    /// The version of the `bootloader_api` crate. Must match the `bootloader` version.
    pub api_version: ApiVersion,
    // /// A map of the physical memory regions of the underlying machine.
    // ///
    // /// The bootloader queries this information from the BIOS/UEFI firmware and translates this
    // /// information to Rust types. It also marks any memory regions that the bootloader uses in
    // /// the memory map before passing it to the kernel. Regions marked as usable can be freely
    // /// used by the kernel.
    // pub memory_regions: &'static MemoryRegions,
    // /// The virtual address at which the mapping of the physical memory starts.
    // ///
    // /// Physical addresses can be converted to virtual addresses by adding this offset to them.
    // ///
    // /// The mapping of the physical memory allows to access arbitrary physical frames. Accessing
    // /// frames that are also mapped at other virtual addresses can easily break memory safety and
    // /// cause undefined behavior. Only frames reported as `USABLE` by the memory map in the `BootInfo`
    // /// can be safely accessed.
    // ///
    // /// Only available if the `map-physical-memory` config option is enabled.
    // pub physical_memory_offset: VirtAddr,
    // /// The virtual address of the recursively mapped level 4 page table.
    // ///
    // /// Only available if the `map-page-table-recursively` config option is enabled.
    // pub recursive_index: Option<u16>,
    /// The address of the `RSDP` data structure, which can be use to find the ACPI tables.
    ///
    /// This field is `None` if no `RSDP` was found (for BIOS) or reported (for UEFI).
    pub rsdp_addr: Option<u64>,
    /// The thread local storage (TLS) template of the kernel executable, if present.
    pub tls_template: Option<TlsTemplate>,
    /// Ramdisk address, if loaded
    pub ramdisk_addr: ReentrantMutex<Option<u64>>,
    /// Ramdisk image size, set to 0 if addr is None
    pub ramdisk_len: u64,
    /// Physical address of the kernel ELF in memory.
    pub kernel_addr: u64,
    /// Size of the kernel ELF in memory.
    pub kernel_len: u64,
    /// Virtual address of the loaded kernel image.
    pub kernel_image_offset: u64,
    pub kernel_elf: KernelElfFile,
    pub eh_info: Option<EhInfo<'static>>,
    pub addr2line: Option<ReentrantMutex<Context<EndianSlice<'static>>>>,

    pub alloc_kinf: &'static ReentrantMutex<AllocKernelInfo>,
    pub stack_alloc: ReentrantMutex<StackAlloc>,
}

impl KernelInfo {
    pub fn create_stack(&self) -> Option<stack::SlabStack> {
        let mut lock = self.alloc_kinf.lock();
        let alloc_kinf = &mut *lock;
        let res = self
            .stack_alloc
            .lock()
            .create_stack(&mut alloc_kinf.page_table, &mut alloc_kinf.frame_allocator);
        drop(lock);
        res
    }

    /// # Safety
    /// The stack shouldn't be used, and the pages used up by it should be unmappable
    pub unsafe fn free_stack(&self, stack: SlabStack) {
        let mut lock = self.alloc_kinf.lock();
        let alloc_kinf = &mut *lock;
        unsafe {
            self.stack_alloc.lock().free_stack(
                stack,
                &mut alloc_kinf.page_table,
                &mut alloc_kinf.frame_allocator,
            )
        }
        drop(lock);
    }

    pub fn create_p4_table_and_switch(
        &self,
    ) -> alloc::sync::Arc<memory::multi_l4_paging::PageTableToken> {
        self.alloc_kinf.lock().create_p4_table_and_switch()
    }
}

pub static KERNEL_INFO: Once<KernelInfo> = Once::new();
static ALLOC_KINF: Once<ReentrantMutex<AllocKernelInfo>> = Once::new();

pub fn setup(boot_info: &'static mut bootloader_api::BootInfo) {
    let layout = memory::range_alloc::discover_layout(boot_info);
    gdt::init();
    interrupts::init_idt();
    if let Some(fb) = boot_info.framebuffer.as_mut() {
        io::framebuffer::init(fb);
    }
    io::serial::init();
    io::logger::init();
    interrupts::init_pics();

    info!("Kernel offset: {:x}", boot_info.kernel_image_offset);
    info!("Kernel physaddr: {:x}", boot_info.kernel_addr);
    info!("Kernel size: {:x}", boot_info.kernel_len);
    info!("Ramdisk addr: {:x?}", boot_info.ramdisk_addr);
    info!("Ramdisk size: {:x}", boot_info.ramdisk_len);

    debug!("Memory regions:");
    let mut usable_bytes = 0;
    let mut usable_kernel_bytes = 0;
    for region in boot_info.memory_regions.iter() {
        debug!(
            " - {:?}: 0x{:X} - 0x{:X}",
            region.kind, region.start, region.end
        );
        if region.kind == MemoryRegionKind::Usable {
            usable_bytes += region.end - region.start;
            if KERNEL_START.as_u64() <= region.end {
                let start = KERNEL_START.as_u64().max(region.start);
                usable_kernel_bytes += region.end - start
            }
        }
    }
    let usable_user_bytes = usable_bytes - usable_kernel_bytes;
    info!("Usable memory summary:");
    info!("Usable memory: {usable_bytes} bytes (0x{usable_bytes:X})");
    info!("Usable kernel memory: {usable_kernel_bytes} bytes (0x{usable_kernel_bytes:X})");
    info!("Usable user memory: {usable_user_bytes} bytes (0x{usable_user_bytes:X})");

    info!("Minimum init done. Setting up memory");

    let physical_memory_offset = VirtAddr::new(
        *boot_info
            .physical_memory_offset
            .as_ref()
            .expect("Physical memory mapped"),
    );
    let page_table = unsafe { memory::init_page_tables(physical_memory_offset) };
    let frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };

    let page_table = PageTables::new(page_table, VirtAddr::new(boot_info.kernel_image_offset));
    info!("Initializing region allocator");
    let virt_region_allocator = memory::range_alloc::init_region_allocator::<_, Size4KiB>(
        &page_table,
        &layout,
        &page_table,
    );
    info!("Initialized region allocator");

    let alloc_kinf = ALLOC_KINF.call_once(|| {
        ReentrantMutex::new(AllocKernelInfo {
            page_table,
            frame_allocator,
            virt_region_allocator,
        })
    });
    info!("Initializing heap");
    allocator::init_heap(alloc_kinf).expect("initialized heap");
    info!("Initialized heap");

    alloc_kinf.lock().frame_allocator.heap_init();

    // unmap userspace pages. Should only be the old gdt mapping
    let mut alloc_kinf_lock = alloc_kinf.lock();
    #[allow(clippy::needless_collect)]
    for page in alloc_kinf_lock
        .page_table
        .mapped_pages_in_range(
            VirtAddr::new(0),
            VirtAddr::new(boot_info.kernel_image_offset),
        )
        .collect::<Vec<_>>()
    {
        trace!(event = "unmap_user_pages", subevent = "before", page:?; "Unmapping {page:?}");
        let (frame, flush) = alloc_kinf_lock.page_table.unmap(page).expect("Unmap page");
        flush.flush();
        trace!(event = "unmap_user_pages", subevent = "after", page:?, frame:?; "Unmapped {page:?} from {frame:?}");
        unsafe { alloc_kinf.lock().frame_allocator.deallocate_frame(frame) };
    }
    drop(alloc_kinf_lock);

    let mut stack_alloc;

    {
        let mut lock = alloc_kinf.lock();
        let locked = &mut *lock;

        stack_alloc = StackAlloc::new(&mut locked.virt_region_allocator);

        let esp0 = stack_alloc
            .create_stack(&mut locked.page_table, &mut locked.frame_allocator)
            .unwrap();
        let ist_df = stack_alloc
            .create_stack(&mut locked.page_table, &mut locked.frame_allocator)
            .unwrap();

        drop(lock);

        gdt::set_tss_guarded_stacks(esp0, ist_df);
    }

    let kernel_elf_slice = unsafe {
        core::slice::from_raw_parts::<'static, _>(
            (physical_memory_offset + boot_info.kernel_addr).as_ptr::<u8>(),
            boot_info.kernel_len as usize,
        )
    };
    let kernel_elf = KernelElfFile::parse(kernel_elf_slice).expect("A valid kernel ELF");

    let eh_info = EhInfo::from_elf(&kernel_elf, boot_info.kernel_image_offset);
    if eh_info.is_none() {
        warn!("No eh_info");
    } else {
        info!("Loaded eh_info");
    }
    let dwarf = load_dwarf(&kernel_elf);
    info!("attempting to load DWARF");

    let addr2line = dwarf
        .inspect_err(|e| warn!("Dwarf error: {e:?}"))
        .ok()
        .and_then(|x| {
            Context::from_dwarf(x)
                .inspect_err(|e| warn!("addr2line error: {e:?}"))
                .ok()
        })
        .map(ReentrantMutex::new);

    let setup_info = KernelInfo {
        kernel_addr: boot_info.kernel_addr,
        api_version: boot_info.api_version,
        // memory_regions: &boot_info.memory_regions,
        // physical_memory_offset,
        // recursive_index: boot_info.recursive_index.as_ref().copied(),
        rsdp_addr: boot_info.rsdp_addr.as_ref().copied(),
        tls_template: boot_info.tls_template.as_ref().copied(),
        ramdisk_addr: ReentrantMutex::new(boot_info.ramdisk_addr.as_ref().copied()),
        ramdisk_len: boot_info.ramdisk_len,
        kernel_len: boot_info.kernel_len,
        kernel_image_offset: boot_info.kernel_image_offset,
        kernel_elf,
        eh_info,
        addr2line,
        alloc_kinf,
        stack_alloc: ReentrantMutex::new(stack_alloc),
    };
    KERNEL_INFO.call_once(|| setup_info);
    multitask::init();
}
