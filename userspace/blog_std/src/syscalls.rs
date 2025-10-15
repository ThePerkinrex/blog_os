macro_rules! generate_syscall {
    ($name:ident, $(($reg_str:tt, $reg:ident),)*) => {
        /// # Safety
        /// The caller must ensure that the syscall number exists, and that it accepts that number of args
        pub unsafe fn $name(code: u64, $($reg: u64,)*) -> u64 {
            let ret: u64;
            unsafe {core::arch::asm!(
                "int 0x80",
                in("rax") code,
                $(
                    // This expands to a list of `in("reg_name") var_name`
                    in($reg_str) $reg,
                )*
                lateout("rax") ret,
                options(nostack)
            )};
            ret
        }
    }
}

macro_rules! generate_syscalls {
    // Internal recursion: stop when no more registers
    (@gen) => {
        generate_syscall!(syscall_arg0, );
    };

    // Internal recursion: consume one register per step
    (@gen ($reg_head_str:literal, $reg_head:ident), $(($reg_tail_str:literal, $reg_tail:ident),)*) => {
        paste::paste! {
        generate_syscall!([<syscall_ $reg_head>], ($reg_head_str, $reg_head), $(($reg_tail_str, $reg_tail),)*);

            // // ... syscall function generation code (omitted for brevity)
            // pub unsafe fn [<syscall_ $reg_head>](code: u64, $reg_head: u64, $($reg_tail: u64,)*) -> u64 {
            //     let ret: u64;
            //     unsafe {core::arch::asm!(
            //         "int 0x80",
            //         in("rax") code,
            //         $(
            //             // This expands to a list of `in("reg_name") var_name`
            //             in($reg_tail_str) $reg_tail,
            //         )*
            //         in($reg_head_str) $reg_head, // The head argument comes last for clarity in the macro
            //         lateout("rax") ret,
            //         options(nostack)
            //     )};
            //     ret
            // }
        }

        // Recurse with incremented N and new args list
        generate_syscalls!(@gen $(($reg_tail_str, $reg_tail),)*); // 👈 CORRECTED RECURSIVE CALL
    };
}

// Usage: generates syscall0..syscall6 automatically
generate_syscalls!(@gen ("r9", arg6), ("r8", arg5), ("r10", arg4), ("rdx", arg3), ("rsi", arg2), ("rdi", arg1),);
