use gimli::{Register, X86_64};

use crate::unwind::UnwinderError;

#[derive(Debug, Default)]
pub struct RegisterSet {
    rip: Option<u64>,
    rsp: Option<u64>,
    rbp: Option<u64>,
    ret: Option<u64>,
}

impl RegisterSet {
    pub fn new(pc: u64) -> Self {
        let mut s = Self::default();
        s.set_pc(pc);
        s
    }

    pub const fn get(&self, reg: Register) -> Option<u64> {
        match reg {
            X86_64::RSP => self.rsp,
            X86_64::RBP => self.rbp,
            X86_64::RA => self.ret,
            _ => None,
        }
    }

    pub const fn set(&mut self, reg: Register, val: u64) -> Result<(), UnwinderError> {
        *match reg {
            X86_64::RSP => &mut self.rsp,
            X86_64::RBP => &mut self.rbp,
            X86_64::RA => &mut self.ret,
            _ => return Err(UnwinderError::UnexpectedRegister(reg)),
        } = Some(val);

        Ok(())
    }

    pub const fn undef(&mut self, reg: Register) {
        // *match reg {
        //     X86_64::RSP => &mut self.rsp,
        //     X86_64::RBP => &mut self.rbp,
        //     X86_64::RA => &mut self.ret,
        //     _ => return,
        // } = None;
    }

    pub const fn get_pc(&self) -> Option<u64> {
        self.rip
    }

    pub const fn set_pc(&mut self, val: u64) {
        self.rip = Some(val);
    }

    pub const fn get_ret(&self) -> Option<u64> {
        self.ret
    }

    pub const fn set_stack_ptr(&mut self, val: u64) {
        self.rsp = Some(val);
    }

    pub fn iter() -> impl Iterator<Item = Register> {
        [X86_64::RSP, X86_64::RBP, X86_64::RA].into_iter()
    }
}
