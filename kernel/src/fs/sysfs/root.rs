use crate::fs::sysfs::{
    device::DevicesINode, driver::DriversINode, proc::ProcsINode,
};

use crate::const_dir;

const_dir!{
    pub struct RootINode {

        dirs = [
            { name: "proc",    inode: ProcsINode },
            { name: "devices", inode: DevicesINode },
            { name: "drivers", inode: DriversINode },
        ];
    }
}
