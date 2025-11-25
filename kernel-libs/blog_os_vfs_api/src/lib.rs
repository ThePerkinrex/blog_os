#![no_std]

extern crate alloc;

pub mod file;
pub mod fs;
pub mod inode;
pub mod path;
pub mod stat;

pub use api_utils::cglue;

#[derive(Debug)]
#[repr(C)]
pub enum IOError {
    NotFound = 0,
    OperationNotPermitted,
    AlreadyExists,
}
