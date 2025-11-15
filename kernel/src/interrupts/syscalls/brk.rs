use log::debug;

pub fn brk(code: u64, offset: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    debug!("BRK SYSCALL ({code})");
    let offset = offset as i64;
    0
}
