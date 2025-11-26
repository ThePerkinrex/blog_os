#![no_std]

use blog_os_vfs::{VFS, api::{file::File, inode::INode, path::PathBuf}};

pub fn load_initcpio(vfs: &mut VFS, ramdisk: &[u8]) {
    for entry in cpio_reader::iter_files(ramdisk) {
        let file = PathBuf::parse(entry.name());

        let file = if file.is_absolute() {
            file
        } else {
            PathBuf::root().join(&file)
        };


        log::debug!("name: {:?} - {} bytes", file, entry.file().len());

        if let Some(parent) = file.parent() {
            vfs.mkdir(parent, true, true).unwrap();
        }

        let inode = vfs.create_file(&file).unwrap();
        let mut opened = inode.open().unwrap();
        let mut data = entry.file();

        loop {
            let written = opened.write(data).unwrap();
            if let Some(d) = data.get(written..) {
                data = d;
            }else{
                break;
            }
        }

        opened.close().unwrap();

        log::debug!("Written");

    }
}
