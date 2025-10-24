use x86_64::VirtAddr;

use crate::println;

pub fn get_backtrace() {
	let rbp: u64;
	unsafe {
		core::arch::asm!("mov rax,rbp", lateout("rax") rbp, options(nostack, nomem));
	}
	let mut rbp = VirtAddr::new(rbp);
	while rbp.as_u64() > 0x1000 {
		let rip = VirtAddr::new(*unsafe { rbp.as_ptr::<u64>().wrapping_add(1).as_ref() }.unwrap());
		println!("rbp: {rbp:p}; rip: {rip:p}");
		rbp = VirtAddr::new(*unsafe{rbp.as_ptr::<u64>().as_ref()}.unwrap());
		if rbp.as_u64() == 1 {
			println!("Reached ih");
		}
	}
}