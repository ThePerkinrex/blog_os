use log::debug;

pub fn nop(arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64, arg6: u64) -> u64 {
    debug!("NOP SYSCALL ({arg1}, {arg2}, {arg3}, {arg4}, {arg5}, {arg6})");
    0
}
