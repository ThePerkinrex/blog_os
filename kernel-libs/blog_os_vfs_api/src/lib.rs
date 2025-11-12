#![no_std]

extern crate alloc;

pub mod file;
pub mod fs;
pub mod inode;
pub mod path;
pub mod stat;

#[repr(C)]
pub enum IOError {
    NotFound = 0,
}
