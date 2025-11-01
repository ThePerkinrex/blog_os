use addr2line::Location;
use alloc::{boxed::Box, collections::binary_heap::BinaryHeap, string::String, sync::Arc};
use kernel_utils::maybe_boxed::MaybeBoxed;

use crate::{elf::LoadedProgram, setup::KernelInfo, unwind::eh::EhInfo};

pub struct ClonedLocation {
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}
impl<'a> From<&Location<'a>> for ClonedLocation {
    fn from(value: &Location<'a>) -> Self {
        Self {
            file: value.file.map(String::from),
            line: value.line,
            column: value.column,
        }
    }
}

impl<'a> From<Location<'a>> for ClonedLocation {
    fn from(value: Location<'a>) -> Self {
        Self {
            file: value.file.map(String::from),
            line: value.line,
            column: value.column,
        }
    }
}

pub trait UnwindableElf {
    fn offset(&self) -> u64;
    fn eh_info(&self) -> Option<&EhInfo<'_>>;

    fn find_location(&self, virtaddr: u64) -> (Result<Option<ClonedLocation>, gimli::Error>, u64);
}

impl UnwindableElf for LoadedProgram {
    fn offset(&self) -> u64 {
        self.load_offset()
    }

    fn eh_info(&self) -> Option<&EhInfo<'_>> {
        // println!("Getting eh_info for process");
        self.eh_info()
    }

    fn find_location(&self, virtaddr: u64) -> (Result<Option<ClonedLocation>, gimli::Error>, u64) {
        let offset = self.offset();
        let addr = virtaddr - offset;
        // println!("Getting location for process: {virtaddr:x} - {offset:x} = {addr:x}");
        (
            self.with_addr2line(|a| a.find_location(addr).map(|x| x.map(ClonedLocation::from)))
                .transpose()
                .map(Option::flatten),
            addr,
        )
    }
}

impl<T: UnwindableElf> UnwindableElf for Arc<T> {
    fn offset(&self) -> u64 {
        self.as_ref().offset()
    }

    fn eh_info(&self) -> Option<&EhInfo<'_>> {
        self.as_ref().eh_info()
    }

    fn find_location(&self, virtaddr: u64) -> (Result<Option<ClonedLocation>, gimli::Error>, u64) {
        self.as_ref().find_location(virtaddr)
    }
}

impl UnwindableElf for KernelInfo {
    fn offset(&self) -> u64 {
        self.kernel_image_offset
    }

    fn eh_info(&self) -> Option<&EhInfo<'_>> {
        self.eh_info.as_ref()
    }

    fn find_location(&self, virtaddr: u64) -> (Result<Option<ClonedLocation>, gimli::Error>, u64) {
        let offset = self.offset();
        let addr = virtaddr - offset;
        let lock = self.addr2line.as_ref().map(|x| x.lock());
        (
            lock.as_ref()
                .map(|x| x.find_location(addr))
                .transpose()
                .map(Option::flatten)
                .map(|x| x.map(Into::into)),
            addr,
        )
    }
}

pub struct OrderedUnwindable<'a>(MaybeBoxed<'a, dyn UnwindableElf>);

impl<'a> UnwindableElf for OrderedUnwindable<'a> {
    fn offset(&self) -> u64 {
        self.0.offset()
    }

    fn eh_info(&self) -> Option<&EhInfo<'_>> {
        self.0.eh_info()
    }

    fn find_location(&self, virtaddr: u64) -> (Result<Option<ClonedLocation>, gimli::Error>, u64) {
        self.0.find_location(virtaddr)
    }
}

impl<'a> Eq for OrderedUnwindable<'a> {}

impl<'a> PartialEq for OrderedUnwindable<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.0.offset() == other.0.offset()
    }
}

impl<'a> PartialOrd for OrderedUnwindable<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for OrderedUnwindable<'a> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.offset().cmp(&other.0.offset())
    }
}

#[derive(Default)]
pub struct UnwindTable<'a> {
    heap: BinaryHeap<OrderedUnwindable<'a>>,
}

impl<'a> UnwindTable<'a> {
    pub fn push_ref<T: UnwindableElf + 'static>(&mut self, r: &'a T, _name: &'static str) {
        self.heap.push(OrderedUnwindable(MaybeBoxed::Borrowed(r)));
    }
    pub fn push_owned<T: UnwindableElf + 'static>(&mut self, o: T, _name: &'static str) {
        self.heap
            .push(OrderedUnwindable(MaybeBoxed::Boxed(Box::new(o))));
    }

    pub fn get(&self, addr: u64) -> Option<&OrderedUnwindable<'a>> {
        self.heap
            .iter()
            .find(|&e| e.offset() < addr)
            .map(|v| v as _)
    }
}
