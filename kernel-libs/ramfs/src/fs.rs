use core::{marker::PhantomData, ptr::NonNull};

use api_utils::cglue;
use blog_os_vfs_api::{
    fs::{Filesystem, cglue_superblock::*},
    path::ffi::PathBufOpaqueRef,
};
use lock_api::RawRwLock;

use crate::superblock::RamFSSuperblock;

pub const RAMFS_TYPE: &str = "ramfs";

#[derive(Debug, Clone, Copy, Default)]
pub struct RamFS<R: RawRwLock + Send + Sync + 'static> {
    _marker: PhantomData<R>,
}

impl<R: RawRwLock + Send + Sync + 'static> Filesystem for RamFS<R> {
    fn name(&self) -> &str {
        RAMFS_TYPE
    }

    fn mount(&self, device: Option<NonNull<PathBufOpaqueRef>>) -> Option<SuperblockBox<'static>> {
        if device.is_some() {
            None
        } else {
            Some(cglue::trait_obj!(
                RamFSSuperblock::<R>::default() as Superblock
            ))
        }
    }
}
