use core::sync::atomic::AtomicBool;

use bootloader_x86_64_common::serial::SerialPort;

use crate::io::STACK;

static STARTED: AtomicBool = AtomicBool::new(false);

pub fn init() {
    if !STARTED.swap(true, core::sync::atomic::Ordering::AcqRel) {
        let serial = unsafe { SerialPort::init() };
        STACK.lock().add(serial).expect("init serial");
    }
}
