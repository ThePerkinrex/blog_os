use core::sync::atomic::AtomicBool;

use log::Record;
use spin::{Lazy, Once};

use crate::io::STACK;

use core::fmt;

pub struct SerialPort {
    port: uart_16550::SerialPort,
}

impl SerialPort {
    /// # Safety
    ///
    /// unsafe because this function must only be called once
    pub unsafe fn init() -> Self {
        unsafe { Self::new(0x3F8) }
    }

    /// # Safety
    ///
    /// unsafe because this function must only be called once for each port
    unsafe fn new(base: u16) -> Self {
        let mut port = unsafe { uart_16550::SerialPort::new(base) };
        port.init();
        Self { port }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for char in s.bytes() {
            match char {
                b'\n' => self.port.write_str("\n").unwrap(),
                byte => self.port.send(byte),
            }
        }
        Ok(())
    }
}

static STARTED: AtomicBool = AtomicBool::new(false);

pub fn init() {
    if !STARTED.swap(true, core::sync::atomic::Ordering::AcqRel) {
        let serial = unsafe { SerialPort::init() };
        STACK.lock().add(serial).expect("init serial");
    }
}

static JSON_SINK: Lazy<SerialPort> = Lazy::new(|| unsafe { SerialPort::new(0x2F8) });
