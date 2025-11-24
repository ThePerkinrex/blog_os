use blog_os_device_api::DeviceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum FileType {
    RegularFile = 0,
    Directory,
    BlockDevice,
    CharDevice,
    SymbolicLink,
    // Socket
    // FIFO
}

#[derive(Debug, PartialEq, Eq, Clone)]
#[repr(C)]
pub struct Stat {
    pub device: Option<DeviceId>,
    pub size: u64,
    pub file_type: FileType, // TODO more stuff
}
