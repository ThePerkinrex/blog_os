use alloc::sync::Arc;
use api_utils::cglue;
use blog_os_vfs::api::{
    IOError,
    file::{File, SeekMode, cglue_file::*},
    inode::{FsINodeRef, INode, cglue_inode::*},
};
use num_enum::TryFromPrimitive;
use shared_fs::{DeviceId, Stat};
use slotmap::Key;

use crate::fs::sysfs::{
    INodes, SysFsINode, device::DevicesINode, driver::DriversINode, proc::ProcsINode,
};

#[repr(u8)]
#[derive(Debug, Clone, Copy, TryFromPrimitive)]
enum Dirs {
    Proc = 0,
    Devices,
    Drivers,
    Max,
}

impl Dirs {
    const fn get_str(&self) -> Option<&'static str> {
        match self {
            Self::Proc => Some("proc"),
            Self::Devices => Some("devices"),
            Self::Drivers => Some("drivers"),
            Self::Max => None,
        }
    }
}

#[derive(Clone)]
struct RootData {
    // inodes: INodes,
    dirs: Arc<[SysFsINode; Dirs::Max as usize]>,
}

pub struct RootINode {
    data: RootData,
}

impl RootINode {
    pub fn new(inodes: INodes) -> Self {
        let mut dirs = [SysFsINode::null(); Dirs::Max as usize];
        let mut lock = inodes.write();

        dirs[Dirs::Proc as usize] = lock.insert(Arc::new(cglue::trait_obj!(ProcsINode::new(
            inodes.clone()
        ) as INode)));
        dirs[Dirs::Devices as usize] = lock.insert(Arc::new(cglue::trait_obj!(DevicesINode::new(
            inodes.clone()
        ) as INode)));
        dirs[Dirs::Drivers as usize] = lock.insert(Arc::new(cglue::trait_obj!(DriversINode::new(
            inodes.clone()
        ) as INode)));

        drop(lock);
        let data = RootData {
            // inodes,
            dirs: Arc::new(dirs),
        };

        Self { data }
    }
}

impl INode for RootINode {
    fn lookup(&self, component: &str) -> Option<FsINodeRef> {
        self.data
            .dirs
            .get(match component {
                "proc" => Some(Dirs::Proc),
                "devices" => Some(Dirs::Devices),
                "drivers" => Some(Dirs::Drivers),
                _ => None,
            }? as usize)
            .map(|x| FsINodeRef(x.data().as_ffi()))
    }

    fn stat(&self) -> Result<Stat, IOError> {
        Ok(Stat {
            device: None,
            size: Dirs::Max as u64,
            file_type: shared_fs::FileType::Directory,
        })
    }

    fn open(&self) -> Result<FileBox<'static>, IOError> {
        Ok(cglue::trait_obj!(RootFile { idx: 0 } as File))
    }
}

struct RootFile {
    idx: u8,
}

impl File for RootFile {
    fn close(&mut self) -> Result<(), IOError> {
        Ok(())
    }

    fn read(&mut self, _: &mut [u8]) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn write(&mut self, _: &[u8]) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn seek(&mut self, _: SeekMode, _: isize) -> Result<usize, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn next_direntry(&mut self) -> Result<&str, IOError> {
        let entry = Dirs::try_from_primitive(self.idx)
            .ok()
            .and_then(|x| x.get_str())
            .ok_or(IOError::EOF)?;
        self.idx = (self.idx + 1).min(Dirs::Max as u8);
        Ok(entry)
    }

    fn mkdir(&mut self, _: &str) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn mknod(&mut self, _: &str, _: DeviceId) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn creat(&mut self, _: &str) -> Result<FsINodeRef, IOError> {
        Err(IOError::OperationNotPermitted)
    }

    fn flush(&mut self) -> Result<(), IOError> {
        Err(IOError::OperationNotPermitted)
    }
}
