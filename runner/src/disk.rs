use std::path::Path;

use bootloader::{BiosBoot, BootConfig, UefiBoot};

pub trait BootBuilder {
    fn set_ramdisk(&mut self, ramdisk_path: &Path);
    fn set_ramdisk_opt(&mut self, ramdisk_path: Option<&Path>) {
        if let Some(ramdisk_path) = ramdisk_path {
            self.set_ramdisk(ramdisk_path);
        }
    }
    fn set_boot_config(&mut self, config: &BootConfig);
    fn create_disk_image(&self, out_path: &Path) -> anyhow::Result<()>;
}

impl BootBuilder for BiosBoot {
    fn set_ramdisk(&mut self, ramdisk_path: &Path) {
        self.set_ramdisk(ramdisk_path);
    }

    fn set_boot_config(&mut self, config: &BootConfig) {
        self.set_boot_config(config);
    }

    fn create_disk_image(&self, out_path: &Path) -> anyhow::Result<()> {
        self.create_disk_image(out_path)
    }
}

impl BootBuilder for UefiBoot {
    fn set_ramdisk(&mut self, ramdisk_path: &Path) {
        self.set_ramdisk(ramdisk_path);
    }

    fn set_boot_config(&mut self, config: &BootConfig) {
        self.set_boot_config(config);
    }

    fn create_disk_image(&self, out_path: &Path) -> anyhow::Result<()> {
        self.create_disk_image(out_path)
    }
}
