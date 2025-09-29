mod framebuffer;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

pub fn print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    WRITER
        .lock()
        .get_mut()
        .expect("A writer")
        .write_fmt(args)
        .expect("write");
}