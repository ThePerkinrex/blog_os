#![no_std]

use blog_os_vfs::VFS;

pub fn load_initcpio(vfs: &mut VFS, ramdisk: &[u8]) {
	for file in cpio_reader::iter_files(ramdisk) {
		log::debug!("name: {:?} - {} bytes", file.name(), file.file().len())
	}
}