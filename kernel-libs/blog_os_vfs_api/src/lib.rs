#![no_std]

extern crate alloc;

pub mod block;
pub mod file;
pub mod fs;
pub mod inode;
pub mod path;

pub enum IOError {
    NotFound,
}
