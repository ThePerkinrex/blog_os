#![no_std]
#![no_main]

use alloc::string::ToString;
use blog_std::{
    fs::{DirIter, FileType},
    init_driver, open,
    path::PathBuf,
    println,
};

extern crate alloc;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Hello, world!");

    blog_std::exit(0)
}