#![no_std]

use x86_64::VirtAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
    PanicWriterFailed = 0x12,
}

pub const KERNEL_START: VirtAddr = VirtAddr::new_truncate(1 << 39); // Leaves 512GB bellow for userspace
