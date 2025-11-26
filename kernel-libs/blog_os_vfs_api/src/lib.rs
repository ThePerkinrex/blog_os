#![no_std]

extern crate alloc;

pub mod file;
pub mod fs;
pub mod inode;
pub mod path;
pub mod stat;

pub use api_utils::cglue;
use thiserror::Error;

#[derive(Debug, Error)]
#[repr(C)]
pub enum IOError {
    #[error("Not found")]
    NotFound = 0,
    #[error("Operation not permitted")]
    OperationNotPermitted,
    #[error("Already exists")]
    AlreadyExists,
}
