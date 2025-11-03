
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

== Code Structure

#lorem(100)

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

= Packages and crates

== `kernel`: The Kernel

#lorem(150)

== `kernel-libs`: Libraries used by the kernel

#lorem(90)

== `qemu-common`: Utilities for interfacing with QEMU and the runner

#lorem(120)


== `runner`: Cargo runner

#lorem(50)

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

