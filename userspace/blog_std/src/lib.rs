#![no_std]

unsafe fn system_interrupt(code: u64, ) {
    unsafe{core::arch::asm!(
        "pushq {code}",
        "int {num}",
        "popq {code}",
        num = const 0x80,
        code = in(reg) code,
        options(nomem, nostack)
    )}
}