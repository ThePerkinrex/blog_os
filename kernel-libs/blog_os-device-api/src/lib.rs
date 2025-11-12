#![no_std]

extern crate alloc;

pub use cglue;

pub mod bus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId {
    pub major: u64,
    pub minor: u64,
}
