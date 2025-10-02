use core::{fmt::Write, ops::DerefMut};

use bootloader_x86_64_common::{framebuffer::FrameBufferWriter, serial::SerialPort};
use spin::{Mutex, MutexGuard};

pub mod framebuffer;
pub mod qemu;
pub mod serial;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[derive(Default)]
enum Writer {
    Framebuffer(FrameBufferWriter),
    Serial(SerialPort),
    #[default]
    None,
}

impl From<FrameBufferWriter> for Writer {
    fn from(value: FrameBufferWriter) -> Self {
        Self::Framebuffer(value)
    }
}
impl From<SerialPort> for Writer {
    fn from(value: SerialPort) -> Self {
        Self::Serial(value)
    }
}

impl core::fmt::Write for Writer {
    fn write_char(&mut self, c: char) -> core::fmt::Result {
        match self {
            Self::Framebuffer(frame_buffer_writer) => frame_buffer_writer.write_char(c),
            Self::Serial(serial) => serial.write_char(c),
            Self::None => Err(core::fmt::Error),
        }
    }

    fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> core::fmt::Result {
        match self {
            Self::Framebuffer(frame_buffer_writer) => frame_buffer_writer.write_fmt(args),
            Self::Serial(serial) => serial.write_fmt(args),
            Self::None => Err(core::fmt::Error),
        }
    }

    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        match self {
            Self::Framebuffer(frame_buffer_writer) => frame_buffer_writer.write_str(s),
            Self::Serial(serial) => serial.write_str(s),
            Self::None => Err(core::fmt::Error),
        }
    }
}

#[derive(Default)]
struct WriteStack {
    stack: [Writer; 2],
    size: usize,
}

impl WriteStack {
    const fn new() -> Self {
        Self {
            size: 0,
            stack: [Writer::None, Writer::None],
        }
    }

    fn add<I: Into<Writer>>(&mut self, w: I) -> Result<(), &'static str> {
        if self.size < self.stack.len() {
            self.stack[self.size] = w.into();
            self.size += 1;
            Ok(())
        } else {
            Err("writer stack is full")
        }
    }

    fn mut_writers(&mut self) -> impl Iterator<Item = &mut Writer> {
        self.stack.iter_mut().take(self.size)
    }

    fn write_fn(&mut self, f: impl Fn(&mut Writer) -> core::fmt::Result) -> core::fmt::Result {
        if self.size == 0 {
            return Err(core::fmt::Error);
        }
        for w in self.mut_writers() {
            f(w)?;
        }
        Ok(())
    }
}

pub struct StackWriter<'a>(&'a mut WriteStack);

impl<'a> core::fmt::Write for StackWriter<'a> {
    fn write_char(&mut self, c: char) -> core::fmt::Result {
        self.0.write_fn(|w| w.write_char(c))
    }

    fn write_fmt(&mut self, args: core::fmt::Arguments<'_>) -> core::fmt::Result {
        self.0.write_fn(|w| w.write_fmt(args))
    }

    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.write_fn(|w| w.write_str(s))
    }
}

static STACK: Mutex<WriteStack> = Mutex::new(WriteStack::new());

pub fn print(args: core::fmt::Arguments) {
    writer(|mut w| w.write_fmt(args).expect("write"));
}

pub fn writer<T, F: FnOnce(StackWriter<'_>) -> T>(f: F) -> T {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut lock = STACK.lock();
        f(StackWriter(lock.deref_mut()))
    })
}
