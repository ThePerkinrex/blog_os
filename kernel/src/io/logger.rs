use blog_os_log::Logger;

pub mod structured;

use crate::{_println, io::serial::print_json};

macro_rules! print {
    ($($arg:tt)*) => ($crate::io::print(format_args!($($arg)*)));
}

fn print_sink(record: &log::Record) {
    print!(
        "[{}][{}] {}\n",
        record.level(),
        record.target(),
        record.args()
    )
}

static LOGGER: Logger<2> = Logger {
    sinks: [print_sink, print_json],
};

pub fn init() {
    _println!("Setting up logger");
    log::set_logger(&LOGGER).expect("the logger");
    log::set_max_level(log::LevelFilter::Trace);
    _println!("Set up logger");
}
