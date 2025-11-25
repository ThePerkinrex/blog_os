#![no_std]

use blog_os_vfs::{VFS, api::path::PathBuf};

pub fn load_initcpio(vfs: &mut VFS, ramdisk: &[u8]) {
    for entry in cpio_reader::iter_files(ramdisk) {
        let file = PathBuf::parse(entry.name());

        let file = if file.is_absolute() {
            file
        } else {
            PathBuf::root().join(&file)
        };

        if let Some(parent) = file.parent() {
            vfs.mkdir(parent, true, true).unwrap();
        }

        log::debug!("name: {:?} - {} bytes", file, entry.file().len())
    }
}
