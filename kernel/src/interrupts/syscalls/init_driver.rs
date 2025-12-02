use alloc::vec::Vec;
use blog_os_vfs::api::{IOError, file::File};
use log::{debug, warn};

use crate::{
    driver::{KDriver, registry::DRIVER_REGISTRY},
    multitask::get_current_process_info,
};

fn init_driver_high_level(fd: u64) -> Result<u64, IOError> {
    let mut x = fd;
    x += 1;

    debug!("print {x}");

    debug!("init driver high level");
    debug!("Loading driver at fd {fd}");

    let file = get_current_process_info()
        .and_then(|pinf| pinf.files().write().remove(fd as usize))
        .ok_or(IOError::NotFound)
        .inspect_err(|e| debug!("Finding fd resulted in error: {e}"))?;

    let mut lock = file.write();

    let mut buf = Vec::new();

    let mut temp = alloc::vec![0u8; 1024 * 128]; // 128 KiB on heap

    loop {
        match lock.read(&mut temp) {
            Ok(bytes) => {
                debug!("Read {bytes} bytes");
                buf.extend_from_slice(&temp[..bytes]);
            }
            Err(IOError::EOF) => break,
            Err(e) => return Err(e),
        }
    }

    lock.close()?;

    drop(lock);

    debug!("Loaded bytes into memory, loading ELF");

    // Read file

    let driver = KDriver::new(&buf)
        .inspect_err(|e| warn!("Error loading driver: {e}"))
        .map_err(|_| IOError::LoadError)?;

    log::info!("Loaded driver {}/{}", driver.name(), driver.version());

    driver.start();

    log::info!("Started driver, saving in registry");
    if let Some(old) = {
        let mut lock = DRIVER_REGISTRY.write();
        let res = lock.insert(driver);
        drop(lock);
        res
    } {
        log::info!(
            "Removed old driver with same name: {}/{}",
            old.name(),
            old.version()
        );
        drop(old);
    }

    Ok(0)
}

pub fn init_driver(fd: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64 {
    debug!("INIT DRIVER ({fd})");
    // let buf = unsafe {
    //     core::slice::from_raw_parts_mut(VirtAddr::new(buf).as_mut_ptr::<u8>(), len as usize)
    // };

    init_driver_high_level(fd).unwrap_or_else(|e| (-(e as i64)) as u64)
}
