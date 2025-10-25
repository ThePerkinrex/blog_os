use alloc::vec::Vec;
use gimli::{CfaRule, EndianSlice, LittleEndian, Register, RegisterRule, UnwindContext, UnwindContextStorage, UnwindSection, UnwindTableRow};

use crate::{println, setup::KERNEL_INFO, unwind::{eh::EhInfo, register::RegisterSet}};

pub mod eh;
mod register;

#[derive(Debug)]
struct CallFrame {
    pub pc: u64,
}


#[derive(Debug)]
enum UnwinderError {
    UnexpectedRegister(Register),
    UnsupportedCfaRule,
    UnimplementedRegisterRule,
    CfaRuleUnknownRegister(Register),
    NoUnwindInfo,
    NoPcRegister,
    NoReturnAddr,
}

pub struct ContextStore;

impl UnwindContextStorage<usize> for ContextStore {
    type Rules = Vec<(Register, RegisterRule<usize>)>;

    type Stack = Vec<UnwindTableRow<usize, Self>>;
}

pub struct Unwinder<'a> {
    eh_info: &'a EhInfo,

    /// A `UnwindContext` needed by Gimli for optimizations.
    unwind_ctx: UnwindContext<usize, ContextStore>,

    /// The current values of registers. These values are updated as we restore
    /// register values.
    regs: RegisterSet,

    /// The current CFA address.
    cfa: u64,

    /// Is it the first iteration?
    is_first: bool,
}

impl<'a> Unwinder<'a> {
    fn new(
        eh_info: &'a EhInfo,
        register_set: RegisterSet,
    ) -> Self {
        Self {
            eh_info,
            unwind_ctx: Default::default(),
            regs: register_set,
            cfa: 0,
            is_first: true,
        }
    }

    fn next(&mut self) -> Result<Option<CallFrame>, UnwinderError> {
        let pc = self.regs.get_pc().ok_or(UnwinderError::NoPcRegister)?;

        if self.is_first {
            println!("IS FIRST");
            self.is_first = false;
            return Ok(Some(CallFrame { pc }));
        }

        let row = self.eh_info.hdr.table().expect("hdr table").unwind_info_for_address(
            &self.eh_info.eh_frame,
            &self.eh_info.base_addrs,
            &mut self.unwind_ctx,
            pc,
            |section, bases, offset| section.cie_from_offset(bases, offset),
        ).map_err(|_| UnwinderError::NoUnwindInfo)?;

        match row.cfa() {
            CfaRule::RegisterAndOffset { register, offset } => {
                let reg_val = self.regs.get(*register)
                    .ok_or(UnwinderError::CfaRuleUnknownRegister(*register))?;
                self.cfa = (reg_val as i64 + offset) as u64;
                println!("Register and offset: {register:?}, {offset:x} = {:x}", self.cfa);
            },
            _ => return Err(UnwinderError::UnsupportedCfaRule),
        }

        for reg in RegisterSet::iter() {
            match row.register(reg) {
                RegisterRule::Undefined => self.regs.undef(reg),
                RegisterRule::SameValue => (),
                RegisterRule::Offset(offset) => {
                    let ptr = (self.cfa as i64 + offset) as u64 as *const usize;
                    println!("OFFSET: {:x} + {offset:x} = {ptr:p}", self.cfa);
                    self.regs.set(reg, unsafe { ptr.read() } as u64)?;
                },
                _ => return Err(UnwinderError::UnimplementedRegisterRule),
            }
        }

        let pc = self.regs.get_ret().ok_or(UnwinderError::NoReturnAddr)? - 1;
        self.regs.set_pc(pc);
        self.regs.set_stack_ptr(self.cfa);

        Ok(Some(CallFrame { pc }))

    }
}


pub fn backtrace() {
    if let Some(eh_info) = KERNEL_INFO.get().unwrap().eh_info.as_ref() {
        let aprox_pc: u64;
        unsafe {
            core::arch::asm!("lea {reg}, [rip]", reg = lateout(reg) aprox_pc, options(nomem,nostack));
        }
        println!("Current pc: {aprox_pc:x}");
        let mut unwind = Unwinder::new(eh_info, RegisterSet::new(aprox_pc));
        println!("Created unwinder");
        loop {
            match unwind.next() {
                Ok(Some(frame)) => {
                    println!("Unwind frame: {:x}", frame.pc)
                }
                Ok(None) => {
                    println!("No stack frame");
                    break;
                }
                Err(e) => {
                    println!("Unwind error: {e:?}");
                    break;
                }
            }
        }
    }else{
        println!("No eh_info -> no available backtracing info");
    }
}