#![no_std]
#![no_main]

use alloc::string::String;
use blog_std::{file::File, fs::DirIter, io::Read, path::PathBuf, println};

extern crate alloc;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    println!("Starting init process");

    for init_dir in DirIter::open(&PathBuf::root())
        .unwrap()
        .filter_map(|x| x.ok())
    {
        println!("[ENTRY] {}", init_dir.name())
    }

    let mut a = File::open(&PathBuf::parse("/a.txt")).unwrap();

    let mut buf = [0; 1024];

    let size = a.read(&mut buf).unwrap();

    println!("REad: {:?}", String::from_utf8_lossy(&buf[..size]));

    drop(a);

    blog_std::exit(0);
}
