use crate::device::DeviceId;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Stat {
    pub device: Option<DeviceId>,
    pub size: u64,
    // TODO more stuff
}
