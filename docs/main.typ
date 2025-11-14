
#import "@preview/basic-report:0.3.1": *

#let version() = {
	let v = sys.inputs.at("version", default: none)
	if v != none {
		[ -- ]
		if v.starts-with("v") or v.starts-with("V") {
			v
		}else{
			raw(v)
		}
	}else{
		[ -- Latest]
	}
}

#show: it => basic-report(
  doc-category: [Documentation#version()],
  doc-title: `blog_os`,
  author: "ThePerkinrex",
  affiliation: none,
  logo: none,
  language: "en",
  compact-mode: false,
  it
)

= Introduction

This is an OS that initially was based on Philipp Oppermann's `blog_os` #footnote[https://os.phil-opp.com/] and was later expanded through different sources, mainly from OSDev Wiki #footnote[https://wiki.osdev.org/Expanded_Main_Page].

// == Code Structure

// #lorem(100)

== Functionality

The kernel currently has the following features:
 - VGA framebuffer printing
 - Serial printing
 - Memory paging
 - Cooperative kernel multitasking with stack switching
 - Userspace process execution in ring3, with ELF loading
 - Userspace calls into the kernel, with syscalls (`int 0x80`), and process switching on those syscalls.
 - Process & task exiting.

The current WIP features are:
 - VFS and user FS API
 - Device buses & PCI
 - Driver API

Future expected features are:
 - StdIO for processes, that could be redirected to different outputs (serial terminals, files...) 
 - Preemptive task switching
 - Advanced task scheduler
 - RAM disk support
 - Devices on FS tree
 - Simple shell & utilities

== Building

To build this OS, the `cargo-make` system is used, so to get a complete OS image, just one command is needed: `cargo make build` at the root of the project. The runner executable will print where the image is located, which will depend on the build profile.

 - For debug builds, use `cargo make build`.
 - For release builds, use `cargo make -p release build`

Other dev utilities are provided by the `cargo-make` system:
 - `cargo make format`: Apply `cargo fmt` to the whole project
 - `cargo make docs`: Apply `cargo doc` to the whole project. Each index for each crate/workspace is printed.
 - `cargo make pdf`: (_`typst` executable is needed_) builds this pdf.

== Running
To run this OS, `cargo-make` can also be used. QEMU for x86-64 needs to be installed.

 - `cargo make run` will start the OS, with a VGA display, and serial output in the terminal.
 - `cargo make -p no_display run` will start the OS, without a VGA display, and serial output in the terminal.
 - `cargo make -p gdb run` will start the OS, without a VGA display, and serial output in the terminal, attaching the `gdb` debugger to it and stopping immediately, _before the bootloader runs and load the kernel in memory._


= Packages and crates

== `kernel`: The Kernel

This crate contains all the main code related with the kernel, and the custom test harness for running it. It is split in many modules:
- `lib.rs`: The base, this contains the initial function for the normal and test execution paths, as well as the panic handlers.
- `main.rs`: It contains the entry point for the OS from the bootloader.
- `allocator.rs`: The heap allocator implementation, based on the _talc_ allocator, with heap growth
- `config.rs`: The static config to be provided to the bootloader for memory mappings.
- `dwarf.rs`: DWARF debug format loading utilities for ELF executables.
- `elf.rs`: ELF loading utilities.
- `fs.rs`: root module for any OS filesystems:
	- `fs/ramfs.rs`: The initial _RamFS_ implementation
- `gdt.rs`: TSS and GDT setup code, and reloading for custom allocated stacks setup with guard pages.
- `interrupts.rs`: Interrupt handlers and setup (PICS, GPF, PF, `int 0x80`...)
	- `interrupts/info.rs`: Static info about the pointers to Interrupt Handlers, for backtracing.
	- `interrupts/syscalls.rs`: _Syscall_ handlers and syscall tail code.
- `io.rs`: All the code related to printing to the screen and serial port. Hopefully it will be replaced somewhat by terminals in the VFS (char devices).
	- `io/framebuffer.rs`: Printing to the VGA framebuffer
	- `io/logging.rs`: Logging infrastructure of the kernel
	- `io/qemu.rs`: Utility for using the QEMU provided io-port for existing with a code.
	- `io/serial.rs`: Printing to the UART serial port
- `memory.rs`: Paging setup and frame allocator/deallocator
	- `memory/multi_l4_paging.rs`: A page table manager that handles multiple L4 page tables with shared kernel tables, and different userspace tables.
	- `memory/pages.rs`: Virtual memory region allocator, to allocate regions of virtual pages, without mapping them, for different usages.
- `multitask.rs`: Kernel task (with multiple kernel stacks) switching infrastructure.
	- `multitask/lock.rs`: Kernel reentrant locking for tasks (without task switching or waiting)
- `priviledge.rs`: Ring 3 jump code from ring 0
- `process.rs`: Data structures and management for userspace processes linked to kernel tasks.
- `progs/`: Copied userspace programs for loading.
- `rand.rs`: PRNG utilities.
- `setup.rs`: Kernel startup code.
- `stack.rs`: Stack creation utility and stack allocator for kernel stacks.
- `unwind.rs`: Backtracing and unwinding infrastructure.
	- `unwind/eh.rs`: Exception handling info extraction from ELFs.
	- `unwind/elf_debug.rs`: Abstractions and helpers for unwinding, in relation with ELF/DWARF info.
	- `unwind/register.rs`: Helper for dealing with register info.

== `kernel-libs`: Libraries used by the kernel

This workspace contains crates that will be used by the kernel, but also some that can also be used by external drivers. This split is useful for code that doesn't necessarily depend on other kernel code, allowing the use of the standard testing framework, and better compile times.


Here there are the following crates:

 - VFS:
  - `blog_os_vfs`: Contains kernel-specific VFS code.
  - `blog_os_vfs_api`: VFS API, for FS drivers.
 - Device:
	- `blog_os-device`: Kernel specific device code, for providing support for the API.
	- `blog_os-device-api`: Device API, for drivers (bus, bus device...)
 - `blog_os-pci`: The PCI bus driver
 - `kernel_utils`: Common utilities used by the kernel, and that can be reused by drivers and other code.
 - `api-utils`: Common types used by APIs (common `cglue` code for FFI).

== `qemu-common`: Utilities for interfacing with QEMU and the runner

This is a common crate shared by the kernel testing framework and the runner, allowing for some communication between them through QEMU-specific APIs.

== `runner`: Cargo runner

This is a utility that is used as a cargo runner and a separate binary. It builds the OS image, bundling together the ELF and the bootloader (either for BIOS or UEFI boot). It also supports starting up `gdb`, with config setup for the kernel, and detecting when testing is going on, for better exit codes. 

== `userspace`: Anything userspace

= Kernel startup

= Simple I/O

= Memory

= Multitasking

= Processes

= Interrupts & Syscalls

= Backtrace, unwinding, & DWARF

= The VFS & FS API

= Userspace API & programs

