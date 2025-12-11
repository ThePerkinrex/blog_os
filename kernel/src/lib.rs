#![no_std]
#![cfg_attr(test, no_main)]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use core::panic::PanicInfo;

use alloc::sync::Arc;
use api_utils::cglue;
use blog_os_vfs::api::{file::cglue_file::*, path::PathBuf};
use log::{debug, info};
use qemu_common::QemuExitCode;
use spin::{Lazy, lock_api::RwLock};

use crate::{
    process::{
        OpenFile, load,
        stdio::{StdIn, StdInData, stderr, stdout},
    },
    setup::KERNEL_INFO,
};

pub mod allocator;
pub mod config;
pub mod dwarf;

pub mod device;
pub mod driver;
#[allow(clippy::future_not_send)]
pub mod elf;
pub mod fs;
pub mod gdt;
pub mod interrupts;
pub mod io;
pub mod memory;
pub mod multitask;
pub mod priviledge;
pub mod process;
pub mod rand;
pub mod setup;
pub mod stack;
pub mod unwind;

static STDIN: Lazy<Arc<RwLock<StdInData>>> =
    Lazy::new(|| Arc::new(RwLock::new(StdInData::default())));

pub fn kernel_main() -> ! {
    // let addresses = [
    //     // the identity-mapped vga buffer page
    //     0xb8000,
    //     // some code page
    //     0x201008,
    //     // some stack page
    //     0x0100_0020_1a10,
    // ];

    // let setup_info = KERNEL_INFO.get().unwrap();
    // let setup_info = setup_info.alloc_kinf.lock();
    // for &address in &addresses {
    //     let virt = VirtAddr::new(address);
    //     // new: use the `mapper.translate_addr` method
    //     let phys = setup_info.page_table.translate_addr(virt);
    //     println!("{:?} -> {:?}", virt, phys);
    // }
    // drop(setup_info);

    // let pci = PciBus::new();

    // for (id, metadata, drv) in pci.connected_devices() {
    //     log::info!("PCI {id} -> {:?} ({metadata})", drv.map(|drv| drv.name()));
    // }

    // hlt_loop();

    // let driver = load_elf(
    //     load_example_driver(),
    //     VirtAddr::new_truncate(30 << 39),
    //     |_, addr| Ok(addr),
    //     false,
    //     KDriverResolver::new(driver::Interface {}),
    // )
    // .unwrap();
    // for s in driver.elf().symbols() {
    //     debug!("driver symbol {:?}", s.name())
    // }

    info!("Loading initramfs");

    fs::init_ramfs();

    debug!("printing root dir");

    // {
    //     let mut lock = VFS.write();

    //     let root = PathBuf::root();
    //     let inode_ref = lock.get_ref(&root).unwrap();
    //     let inode = lock.get_inode(inode_ref).unwrap();
    //     debug!(
    //         "root inode ({inode_ref:?}: {}): {:?}",
    //         root,
    //         inode.stat().unwrap()
    //     );

    //     let file = inode.open().unwrap();
    //     for inode in file.readdir().unwrap() {
    //         let path = root.join(&PathBuf::parse(&inode));
    //         debug!("subpath: {inode:?}: {}", path)
    //     }

    //     drop(lock);
    // }

    // hlt_loop();

    // info!("Adding new task to list");
    // multitask::create_task(other_task, "other_task");

    // info!("DID NOT CRASH!");
    // info!("Switching");
    // multitask::task_switch_safe();
    // info!("Returned");
    // info!("Going back");
    // multitask::task_switch_safe();

    // multitask::create_task(switch_loop, "switch loop");
    // multitask::create_task(second_process, "second_process");

    // println!("JUmping to user mode");
    // test_jmp_to_usermode();

    let p = load(&PathBuf::parse("/init")).unwrap();

    p.files()
        .write()
        .insert(Arc::new(RwLock::new(OpenFile::new_no_inode(
            cglue::trait_obj!(StdIn::new(STDIN.clone()) as File),
        ))));
    p.files()
        .write()
        .insert(Arc::new(RwLock::new(OpenFile::new_no_inode(
            cglue::trait_obj!(stdout() as File),
        ))));
    p.files()
        .write()
        .insert(Arc::new(RwLock::new(OpenFile::new_no_inode(
            cglue::trait_obj!(stderr() as File),
        ))));

    p.start();

    // let prog = elf::load_example_elf();
    // let proc = ProcessInfo::new(prog);
    // proc.start();

    hlt_loop();
}

// pub extern "C" fn other_task() {
//     info!("STarted other task");
//     info!("Switching back");
//     multitask::task_switch_safe();
//     info!("Should not be here");
// }

// pub extern "C" fn second_process() {
//     info!("Starting second process");
//     let prog = elf::load_example_elf();
//     let proc = ProcessInfo::new(prog);
//     proc.start();
// }

// pub extern "C" fn test_return() -> ! {
//     info!("REturned here");
//     hlt_loop();
// }

pub extern "C" fn switch_loop() {
    x86_64::instructions::interrupts::enable();
    loop {
        info!("SWITCH LOOP - Waiting");
        x86_64::instructions::interrupts::enable_and_hlt();
        info!("SWITCH LOOP - Switching");
        multitask::task_switch();
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
        _print!("{}...\t", core::any::type_name::<T>());
        self();
        _println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    info!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    io::qemu::exit_qemu(QemuExitCode::Success);
}

pub fn panic_handler(info: &PanicInfo) -> ! {
    log::error!("{info}");
    // if io::writer(|mut w| writeln!(w, "{info}")).is_err() {
    //     io::qemu::exit_qemu(QemuExitCode::PanicWriterFailed);
    // }
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
    setup::setup(boot_info);

    kernel_test()
}

#[cfg(test)]
bootloader_api::entry_point!(kernel_test_entrypoint, config = &config::BOOTLOADER_CONFIG);

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}
