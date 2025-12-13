use alloc::{format, string::ToString, vec::Vec};
use gimli::{
    CfaRule, Register, RegisterRule, UnwindContext, UnwindContextStorage, UnwindSection,
    UnwindTableRow, X86_64,
};
use log::{debug, error, info, trace, warn};
use x86_64::{
    VirtAddr,
    structures::paging::{PageSize, Size4KiB, Translate},
};

use crate::{
    interrupts::info::IH,
    multitask::try_get_current_process_info,
    setup::KERNEL_INFO,
    unwind::{
        elf_debug::{OrderedUnwindable, UnwindTable, UnwindableElf},
        register::RegisterSet,
    },
};

pub mod eh;
pub mod elf_debug;
mod register;

#[derive(Debug, Clone, Copy)]
struct CallFrame {
    pub pc: u64,
    pub sp: u64,
}

#[derive(Debug)]
#[allow(dead_code)]
enum UnwinderError {
    UnexpectedRegister(Register),
    UnsupportedCfaRule,
    UnimplementedRegisterRule,
    CfaRuleUnknownRegister(Register),
    NoUnwindInfo(u64),
    NoPcRegister,
    NoReturnAddr,
    InvalidAddr,
}

pub struct ContextStore;

impl UnwindContextStorage<usize> for ContextStore {
    type Rules = Vec<(Register, RegisterRule<usize>)>;

    type Stack = Vec<UnwindTableRow<usize, Self>>;
}

pub struct Unwinder<'a> {
    unwind: UnwindTable<'a>,

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
    fn new(unwind_table: UnwindTable<'a>, register_set: RegisterSet) -> Self {
        Self {
            unwind: unwind_table,
            unwind_ctx: Default::default(),
            regs: register_set,
            cfa: 0,
            is_first: true,
        }
    }

    fn current_pc(&self) -> Result<u64, UnwinderError> {
        self.regs.get_pc().ok_or(UnwinderError::NoPcRegister)
    }

    fn current_unwindable(&self) -> Result<Option<&OrderedUnwindable<'a>>, UnwinderError> {
        Ok(self.unwind.get(self.current_pc()?))
    }

    fn next(&mut self) -> Result<Option<CallFrame>, UnwinderError> {
        let pc = self.current_pc()?;
        // debug!("Current pc: 0x{pc:X}");
        let unwindable = self.unwind.get(pc).ok_or(UnwinderError::NoUnwindInfo(pc))?;
        // debug!("Found unwindable");
        let eh_info = unwindable
            .eh_info()
            .ok_or(UnwinderError::NoUnwindInfo(pc))?;
        // debug!("Found got eh_info");

        // println!("Loaded eh_info");

        if self.is_first {
            debug!("IS FIRST");
            self.is_first = false;
            return Ok(Some(CallFrame {
                pc,
                sp: self.regs.get(X86_64::RSP).unwrap_or(self.cfa),
            }));
        }

        let fde = eh_info
            .hdr
            .table()
            .expect("hdr table")
            .fde_for_address(
                &eh_info.eh_frame,
                &eh_info.base_addrs,
                pc,
                |section, bases, offset| section.cie_from_offset(bases, offset),
            )
            .map_err(|e| {
                error!("Unwind error: {e}");
                UnwinderError::NoUnwindInfo(pc)
            })?;

        // println!("Gotten fde");

        let row = fde
            .unwind_info_for_address(
                &eh_info.eh_frame,
                &eh_info.base_addrs,
                &mut self.unwind_ctx,
                pc,
            )
            .map_err(|e| {
                error!("Unwind error: {e}");
                UnwinderError::NoUnwindInfo(pc)
            })?;
        // println!("Gotten row");

        match row.cfa() {
            CfaRule::RegisterAndOffset { register, offset } => {
                // println!(
                //     "Register and offset: CFA = {register:?} + {offset:x} = {:x}",
                //     self.cfa
                // );
                let reg_val = self
                    .regs
                    .get(*register)
                    .ok_or(UnwinderError::CfaRuleUnknownRegister(*register))?;
                self.cfa = (reg_val as i64 + offset) as u64;
            }
            _ => return Err(UnwinderError::UnsupportedCfaRule),
        }
        // println!("Applied CFA rules");

        for reg in RegisterSet::iter() {
            let rule = row.register(reg);
            // println!("Rule for {reg:?}: {rule:?}");
            match rule {
                RegisterRule::Undefined => self.regs.undef(reg),
                RegisterRule::SameValue => (),
                RegisterRule::Offset(offset) => {
                    let ptr = (self.cfa as i64 + offset) as u64 as *const usize;
                    // println!("OFFSET: {reg:?}: {:x} + {offset:x} = {ptr:p}", self.cfa);
                    // Check if weve reached the bottom of stack.
                    // Most probable culprit: bottom of process stack
                    if let Some(pinf) = try_get_current_process_info() {
                        let ptr = VirtAddr::from_ptr(ptr);
                        let top = pinf.program().stack().top();
                        if ptr >= top && ptr < top + Size4KiB::SIZE {
                            // Within guard page
                            trace!("REGISTER READ WITHIN PROCESS GUARD PAGE");
                            return Err(UnwinderError::NoUnwindInfo(pc));
                        }
                    }

                    let lock = KERNEL_INFO.get().unwrap().alloc_kinf.lock();

                    if lock
                        .page_table
                        .translate_addr(VirtAddr::from_ptr(ptr))
                        .is_none()
                    {
                        warn!("The saved register ({reg:?}) ptr is not mapped: {ptr:p}");
                        return Err(UnwinderError::InvalidAddr);
                    }

                    self.regs.set(reg, unsafe { ptr.read() } as u64)?;
                    drop(lock);
                }
                _ => return Err(UnwinderError::UnimplementedRegisterRule),
            }
        }
        // println!("Applied register rules");

        let start = VirtAddr::new(fde.initial_address());

        if let Some(i) = IH.get(&start) {
            debug!("IH: {i:?}");
            let saved_cs_ptr = (self.cfa) as *const u64;
            let lock = KERNEL_INFO.get().unwrap().alloc_kinf.lock();
            if lock
                .page_table
                .translate_addr(VirtAddr::from_ptr(saved_cs_ptr))
                .is_none()
            {
                warn!("The saved CS ptr is not mapped: {saved_cs_ptr:p}");
                return Err(UnwinderError::InvalidAddr);
            }

            let saved_cs = unsafe { saved_cs_ptr.read() };

            debug!("Saved cs: {saved_cs:x} (CPL: {:x})", saved_cs & 0x3);

            let cpl = saved_cs & 0x3;
            if cpl == 3 {
                // Came from user mode → CPU pushed RSP
                let saved_rsp_ptr = (self.cfa + 16) as *const u64;
                if lock
                    .page_table
                    .translate_addr(VirtAddr::from_ptr(saved_rsp_ptr))
                    .is_none()
                {
                    warn!("The saved RSP ptr is not mapped: {saved_rsp_ptr:p}");
                    return Err(UnwinderError::InvalidAddr);
                }
                let saved_rsp = unsafe { saved_rsp_ptr.read() };

                debug!("Interrupt return to ring3, saved RSP = {saved_rsp:x}");

                self.cfa = saved_rsp;
                // self.regs.set_stack_ptr(saved_rsp);
                // let _ = self.regs.set(X86_64::RSP, saved_rsp);
            } else {
                // TODO is this correct?
                debug!("Interrupt return to ring0, no saved RSP");
            }
            drop(lock);
        }

        // println!("Updated regs: {:x?}", self.regs);

        let pc = self
            .regs
            .get_ret()
            .ok_or(UnwinderError::NoReturnAddr)?
            .saturating_sub(1);
        self.regs.set_pc(pc);
        self.regs.set_stack_ptr(self.cfa);
        // println!("Set regs");

        debug!("PC: 0x{pc:X}; RSP: 0x{:X}", self.cfa);

        Ok(Some(CallFrame { pc, sp: self.cfa }))
    }
}

fn single_backtrace_line(frame: CallFrame, unwind: &Unwinder<'_>) -> Result<(), UnwinderError> {
    let unwindable = unwind
        .current_unwindable()?
        .ok_or(UnwinderError::NoUnwindInfo(frame.pc))?;

    let (location, addr) = unwindable.find_location(frame.pc);

    let location = location
        .inspect_err(|e| warn!("No location information: {e}"))
        .ok()
        .flatten();

    // Build location info string
    let loc_str = if let Some(location) = location {
        let file = location.file.unwrap_or_else(|| "<unknown file>".into());
        let line = location
            .line
            .map(|l| l.to_string())
            .unwrap_or_else(|| "<unknown line>".into());
        let col = location
            .column
            .map(|c| c.to_string())
            .unwrap_or_else(|| "<unknown column>".into());

        format!("{file}:{line}:{col}")
    } else {
        "<unknown>".into()
    };

    info!(
        "Unwind frame: sp: {:#x}; ip: {:#x} (elf: {:#x}) {}",
        frame.sp, frame.pc, addr, loc_str
    );

    Ok(())
}

pub fn backtrace() {

    let aprox_pc: u64;
    let sp: u64;
    unsafe {
        core::arch::asm!("
            lea {pc}, [rip]
            mov {sp}, rsp
            ", pc = lateout(reg) aprox_pc, sp = lateout(reg) sp, options(nomem,nostack));
    }
    backtrace_sp_ip(sp, aprox_pc);
}

pub fn backtrace_sp_ip(sp: u64, pc: u64) {
    let kinf = KERNEL_INFO.get().unwrap();
    let mut unwind_table = UnwindTable::default();
    unwind_table.push_ref(kinf, "kernel");

    if let Some(pinf) = try_get_current_process_info() {
        unwind_table.push_owned(pinf.program().clone(), "process");
    }
    debug!("Current pc: {pc:x}");
    let mut register_set = RegisterSet::new(pc);
    register_set.set_stack_ptr(sp);
    let mut unwind = Unwinder::new(unwind_table, register_set);
    debug!("Created unwinder");
    loop {
        // println!("UNWIND REGS: {:x?}", unwind.regs);
        match unwind.next() {
            Ok(Some(frame)) => {
                if let Err(e) = single_backtrace_line(frame, &unwind) {
                    info!("Unwind frame: sp: {:#x}; ip: {:#x}", frame.sp, frame.pc);
                    warn!("{e:x?}")
                }
            }
            Ok(None) => {
                info!("No stack frame");
                break;
            }
            Err(e) => {
                warn!("Unwind error: {e:x?}");
                break;
            }
        }
    }
}