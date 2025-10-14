use crate::println;

type SyscallHandler = fn(u64, u64, u64, u64, u64, u64) -> u64;

const SYSCALL_HANDLERS: &[SyscallHandler] = &[nop, exit];

fn nop(arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64, arg6: u64) -> u64 {
	println!("NOP SYSCALL ({arg1}, {arg2}, {arg3}, {arg4}, {arg5}, {arg6})");
	0
}


fn exit(code: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
	println!("EXIT SYSCALL ({code})");
	1
}

pub fn syscall_handle(code: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64, arg6: u64) -> u64 {
	if code < SYSCALL_HANDLERS.len() as u64 {
		SYSCALL_HANDLERS[code as usize](arg1, arg2, arg3, arg4, arg5, arg6)
	}else{
		println!("Unknown syscall");
		u64::MAX
	}
}