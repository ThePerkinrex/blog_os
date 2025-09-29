use core::cell::OnceCell;

use bootloader_api::info::FrameBuffer;
use bootloader_x86_64_common::framebuffer::FrameBufferWriter;
use spin::Mutex;

pub(super) static FB_WRITER: Mutex<OnceCell<FrameBufferWriter>> = Mutex::new(OnceCell::new());

pub fn init(fb: &'static mut FrameBuffer) {
    let info = fb.info();
    let buffer = fb.buffer_mut();
    let fb_writer = FrameBufferWriter::new(buffer, info);
    let _ = FB_WRITER.lock().set(fb_writer);
}
