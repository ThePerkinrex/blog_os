#![no_std]
#![cfg_attr(test, no_main)]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::panic::PanicInfo;

use alloc::boxed::Box;
use bootloader_api::{config::ApiVersion, info::TlsTemplate};
use qemu_common::QemuExitCode;
use spin::{Mutex, Once};
use x86_64::{VirtAddr, structures::paging::Translate};

use crate::{
    memory::{BootInfoFrameAllocator, multi_l4_paging::PageTables, pages::VirtRegionAllocator},
    process::ProcessInfo,
    stack::StackAlloc,
};

pub mod allocator;
pub mod config;
pub mod elf;
pub mod gdt;
pub mod interrupts;
pub mod io;
pub mod memory;
pub mod multitask;
pub mod priviledge;
pub mod process;
pub mod stack;
pub mod util;
pub mod unwind;

pub struct SetupInfo {
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
    pub ramdisk_addr: Option<u64>,
    /// Ramdisk image size, set to 0 if addr is None
    pub ramdisk_len: u64,
    /// Physical address of the kernel ELF in memory.
    pub kernel_addr: u64,
    /// Size of the kernel ELF in memory.
    pub kernel_len: u64,
    /// Virtual address of the loaded kernel image.
    pub kernel_image_offset: u64,
    // The kernel page tables
    pub page_table: PageTables,
    pub frame_allocator: BootInfoFrameAllocator,
    pub virt_region_allocator: VirtRegionAllocator<1>,
    pub stack_alloc: StackAlloc,
}

impl SetupInfo {
    pub fn create_stack(&mut self) -> Option<stack::SlabStack> {
        self.stack_alloc
            .create_stack(&mut self.page_table, &mut self.frame_allocator)
    }

    pub fn create_p4_table_and_switch(&mut self) {
        self.page_table
            .create_process_p4_and_switch(&mut self.frame_allocator);
    }
}

static KERNEL_INFO: Once<Mutex<SetupInfo>> = Once::new();

pub fn setup(boot_info: &'static mut bootloader_api::BootInfo) {
    let layout = memory::pages::discover_layout(boot_info);
    gdt::init();
    interrupts::init_idt();
    if let Some(fb) = boot_info.framebuffer.as_mut() {
        io::framebuffer::init(fb);
    }
    io::serial::init();
    interrupts::init_pics();

    println!("Minimum init done. Setting up memory");

    let physical_memory_offset = VirtAddr::new(
        *boot_info
            .physical_memory_offset
            .as_ref()
            .expect("Physical memory mapped"),
    );
    let mut page_table = unsafe { memory::init_page_tables(physical_memory_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_regions) };
    println!("Initializing region allocator");
    let mut virt_region_allocator = memory::pages::init_region_allocator(&layout, &page_table);
    println!("Initializing heap");
    allocator::init_heap(
        &mut page_table,
        &mut frame_allocator,
        &mut virt_region_allocator,
    )
    .expect("initialized heap");
    let mut stack_alloc = StackAlloc::new(&mut virt_region_allocator);

    let esp0 = stack_alloc
        .create_stack(&mut page_table, &mut frame_allocator)
        .unwrap();
    let ist_df = stack_alloc
        .create_stack(&mut page_table, &mut frame_allocator)
        .unwrap();

    gdt::set_tss_guarded_stacks(esp0, ist_df);

    let setup_info = SetupInfo {
        kernel_addr: boot_info.kernel_addr,
        api_version: boot_info.api_version,
        // memory_regions: &boot_info.memory_regions,
        // physical_memory_offset,
        // recursive_index: boot_info.recursive_index.as_ref().copied(),
        rsdp_addr: boot_info.rsdp_addr.as_ref().copied(),
        tls_template: boot_info.tls_template.as_ref().copied(),
        ramdisk_addr: boot_info.ramdisk_addr.as_ref().copied(),
        ramdisk_len: boot_info.ramdisk_len,
        kernel_len: boot_info.kernel_len,
        kernel_image_offset: boot_info.kernel_image_offset,
        page_table: PageTables::new(page_table),
        frame_allocator,
        virt_region_allocator,
        stack_alloc,
    };
    KERNEL_INFO.call_once(|| Mutex::new(setup_info));
    multitask::init();
}

pub fn kernel_main() -> ! {
    println!("HELLO");

    let addresses = [
        // the identity-mapped vga buffer page
        0xb8000,
        // some code page
        0x201008,
        // some stack page
        0x0100_0020_1a10,
    ];

    for &address in &addresses {
        let setup_info = KERNEL_INFO.get().unwrap().lock();
        let virt = VirtAddr::new(address);
        // new: use the `mapper.translate_addr` method
        let phys = setup_info.page_table.translate_addr(virt);
        println!("{:?} -> {:?}", virt, phys);
    }

    let x = Box::new(41);
    println!("heap_value at {x:p}");

    println!("Adding new task to list");
    multitask::create_task(other_task, "other_task");

    println!("DID NOT CRASH!");
    println!("Switching");
    multitask::task_switch_safe();
    println!("Returned");
    println!("Going back");
    multitask::task_switch_safe();

    multitask::create_task(switch_loop, "switch loop");
    multitask::create_task(second_process, "second_process");

    // println!("JUmping to user mode");
    // test_jmp_to_usermode();
    let prog = elf::load_example_elf();
    let proc = ProcessInfo::new(prog);
    proc.start();
    hlt_loop()
}

pub extern "C" fn other_task() {
    println!("STarted other task");
    println!("Switching back");
    multitask::task_switch_safe();
    println!("Should not be here");
}

pub extern "C" fn second_process() {
    println!("Starting second process");
    let prog = elf::load_example_elf();
    let proc = ProcessInfo::new(prog);
    proc.start();
}

pub extern "C" fn test_return() -> ! {
    println!("REturned here");
    hlt_loop();
}

pub extern "C" fn switch_loop() {
    x86_64::instructions::interrupts::enable();
    loop {
        println!("SWITCH LOOP");
        x86_64::instructions::hlt();
        multitask::task_switch_safe();
    }
}

#[cfg(test)]
pub fn kernel_test() -> ! {
    test_main();
    hlt_loop()
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        print!("{}...\t", core::any::type_name::<T>());
        self();
        println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    io::qemu::exit_qemu(QemuExitCode::Success);
}

pub fn panic_handler(info: &PanicInfo) -> ! {
    use core::fmt::Write;

    if io::writer(|mut w| writeln!(w, "{info}")).is_err() {
        io::qemu::exit_qemu(QemuExitCode::PanicWriterFailed);
    }
    hlt_loop()
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    use core::fmt::Write;

    use crate::io::qemu::exit_qemu;

    if io::writer(|mut w| writeln!(w, "[failed]\n")).is_err() {
        io::qemu::exit_qemu(QemuExitCode::PanicWriterFailed);
    }

    if io::writer(|mut w| writeln!(w, "Error: {info}\n")).is_err() {
        io::qemu::exit_qemu(QemuExitCode::PanicWriterFailed);
    }
    exit_qemu(QemuExitCode::Failed);
    hlt_loop()
}

#[cfg(test)]
pub fn kernel_test_entrypoint(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    setup(boot_info);

    kernel_test()
}

#[cfg(test)]
bootloader_api::entry_point!(kernel_test_entrypoint, config = &config::BOOTLOADER_CONFIG);

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}
