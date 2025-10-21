#![no_std]

mod syscalls;

pub fn nop(code: u64) {
    unsafe { syscalls::syscall_arg1(0x0, code) };
}

pub fn exit(code: u64) {
    let ret = unsafe { syscalls::syscall_arg1(0x1, code) };
    nop(ret);
}
