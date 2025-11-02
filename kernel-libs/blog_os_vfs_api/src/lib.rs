#![no_std]

extern crate alloc;

pub mod device;
pub mod file;
pub mod fs;
pub mod inode;
pub mod path;
pub mod stat;

pub enum IOError {
    NotFound,
}
