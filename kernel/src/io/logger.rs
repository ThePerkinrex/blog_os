use crate::_println;

macro_rules! print {
    ($($arg:tt)*) => ($crate::io::print(format_args!($($arg)*)));
}

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            print!(
                "[{}][{}] {}\n",
                record.level(),
                record.target(),
                record.args()
            )
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

pub fn init() {
    _println!("Setting up logger");
    log::set_logger(&LOGGER).expect("the logger");
    log::set_max_level(log::LevelFilter::Trace);
    _println!("Set up logger");
}
