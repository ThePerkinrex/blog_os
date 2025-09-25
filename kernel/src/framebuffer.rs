use core::cell::OnceCell;

use bootloader_api::info::FrameBuffer;
use bootloader_x86_64_common::framebuffer::FrameBufferWriter;
use spin::Mutex;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::framebuffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

static WRITER: Mutex<OnceCell<FrameBufferWriter>> = Mutex::new(OnceCell::new());

pub fn init(fb: &'static mut FrameBuffer) {
    let info = fb.info();
    let buffer = fb.buffer_mut();
    let fb_writer = FrameBufferWriter::new(buffer, info);
    let _ = WRITER.lock().set(fb_writer);
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    WRITER
        .lock()
        .get_mut()
        .expect("A writer")
        .write_fmt(args)
        .expect("write");
}
