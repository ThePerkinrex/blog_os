#![no_std]

mod syscalls;

pub fn exit(code: u64) {
    let ret = unsafe {syscalls::syscall_arg1(0x1, code)};
    let _ = unsafe {syscalls::syscall_arg1(0x0, ret)};
}