
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

#lorem(120)

#lorem(150)

== Code Structure

#lorem(100)

== Functionality

#lorem(80)

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

