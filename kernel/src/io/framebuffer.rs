use bootloader_api::info::FrameBuffer;
use bootloader_x86_64_common::framebuffer::FrameBufferWriter;

use crate::io::STACK;

pub fn init(fb: &'static mut FrameBuffer) {
    let info = fb.info();
    let buffer = fb.buffer_mut();
    let fb_writer = FrameBufferWriter::new(buffer, info);
    STACK.lock().add(fb_writer).expect("init fb");
}
