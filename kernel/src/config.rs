use bootloader_api::config::{BootloaderConfig, Mapping, Mappings};
use qemu_common::KERNEL_START;

// Define a new Mappings struct based on the default
const MAPPINGS_CONFIG: Mappings = {
    let mut mappings = Mappings::new_default();

    // Set the physical_memory field
    // This is the equivalent of the `map_physical_memory` feature
    mappings.physical_memory = Some(Mapping::Dynamic);
    mappings.kernel_base = Mapping::FixedAddress(KERNEL_START.as_u64());
    mappings.dynamic_range_start = Some(KERNEL_START.as_u64()); // leave bottom half empty

    // Optionally, you can set a fixed virtual address for the physical map:
    // mappings.physical_memory = Some(Mapping::FixedAddress(0xFFFF_8000_0000_0000));

    mappings
};

// Define the overall BootloaderConfig
pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings = MAPPINGS_CONFIG;
    // Set other configuration options if needed, e.g., stack size
    // config.kernel_stack_size = 100 * 1024; // 100 KiB
    config
};
