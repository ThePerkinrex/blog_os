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
    println!("Starting init process");

    let path = PathBuf::parse("/drivers");
    for init_dir in DirIter::open(&path).unwrap().filter_map(|x| x.ok()) {
        let subpath = path.join(&PathBuf::parse(init_dir.name().as_ref()));
        println!("[ENTRY] {subpath}");

        let path_string = subpath.to_string();
        let stat = blog_std::stat(&subpath).unwrap();
        let is_driver =
            stat.file_type == blog_std::fs::FileType::RegularFile && path_string.ends_with(".ko");

        println!(
            "Path: {path_string:?}, Stat: {:?}, is_driver: {}; type: {}, ending: {}",
            stat,
            is_driver,
            stat.file_type == FileType::RegularFile,
            path_string.ends_with(".ko")
        );
        if is_driver {
            println!("Loading driver at path {path}");
            let fd = open(&subpath).unwrap();
            init_driver(fd).unwrap();
        }
    }

    blog_std::exit(0);
}
