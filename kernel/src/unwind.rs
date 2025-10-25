use core::ptr;

use x86_64::VirtAddr;

use crate::println;

pub fn get_backtrace() {
    // Read RBP into a u64
    let mut rbp_val: u64;
    unsafe {
        core::arch::asm!("mov {}, rbp", out(reg) rbp_val, options(nomem, nostack));
    }

    let mut rbp = VirtAddr::new(rbp_val);

    while rbp.as_u64() > 0x1000 {
        // Addresses we will read
        let rbp_ptr = rbp.as_u64().wrapping_add(8) as *const u64;
        let rip_ptr = rbp.as_u64().wrapping_add(16) as *const u64;

        // SAFELY read the qwords (may still page-fault in kernel if unmapped)
        let prev_rbp_val = unsafe { ptr::read_unaligned(rbp_ptr) };
        let rip_val = unsafe { ptr::read_unaligned(rip_ptr) };

        println!(
            "frame addr: {rbp:p} ; [rbp] = {prev_rbp_val:#018x} ; [rbp+8] = {rip_val:#018x}"
        );

        // If we hit your interrupt marker (saved rbp was set to point to marker),
        // detect it and handle accordingly.
        if prev_rbp_val == 1 {
            println!("Reached interrupt-handler marker at {rbp:p}; rip = {rip_val:#018x}");
            break; // or continue into user stack if you implement that
        }

        // Some safety checks to avoid infinite loops
        if prev_rbp_val == 0 || prev_rbp_val == rbp.as_u64() || prev_rbp_val < 0x1000 {
            println!("Stopping unwinding: invalid next rbp {prev_rbp_val:#x}");
            break;
        }

        // Advance to next frame
        if let Ok(new_rbp) = VirtAddr::try_new(prev_rbp_val) {
			rbp = new_rbp;
		}else{
            println!("Stopping unwinding: invalid next rbp {prev_rbp_val:#x}");
			break;
		}
    }
}