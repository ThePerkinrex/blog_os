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
use x86_64::{structures::paging::{OffsetPageTable, Translate}, VirtAddr};

use crate::memory::BootInfoFrameAllocator;

pub mod gdt;
pub mod interrupts;
pub mod io;
pub mod config;
pub mod memory;
pub mod allocator;

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
    pub page_table: OffsetPageTable<'static>,
    pub frame_allocator: BootInfoFrameAllocator 
}

pub fn setup(boot_info: &'static mut bootloader_api::BootInfo) -> SetupInfo {
    gdt::init();
    interrupts::init_idt();
    if let Some(fb) = boot_info.framebuffer.as_mut() {
        io::framebuffer::init(fb);
    }
    io::serial::init();
    interrupts::init_pics();

    let physical_memory_offset = VirtAddr::new(*boot_info.physical_memory_offset.as_ref().expect("Physical memory mapped"));
    let mut page_table = unsafe { memory::init(physical_memory_offset) };
    let mut frame_allocator = unsafe {BootInfoFrameAllocator::init(&boot_info.memory_regions)};
    allocator::init_heap(&mut page_table, &mut frame_allocator).expect("initialized heap");
    SetupInfo { 
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
        page_table,
        frame_allocator
        
    }
}

pub fn kernel_main(setup_info: SetupInfo) -> ! {
    println!("HELLO");

    let addresses = [
        // the identity-mapped vga buffer page
        0xb8000,
        // some code page
        0x201008,
        // some stack page
        0x0100_0020_1a10
    ];

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        // new: use the `mapper.translate_addr` method
        let phys = setup_info.page_table.translate_addr(virt);
        println!("{:?} -> {:?}", virt, phys);
    }

    let x = Box::new(41);
    println!("heap_value at {x:p}");

    println!("DID NOT CRASH!");
    hlt_loop()
}

pub extern "C" fn test_return() -> ! {
    println!("REturned here");
    hlt_loop();
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
