#![no_std]

pub struct Logger<const N: usize, T> {
    pub sinks: [fn(&log::Record, T); N],
    pub transform: fn(&log::Record) -> T,
}

impl<const N: usize, T> log::Log for Logger<N, T> {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            for sink in &self.sinks {
                (sink)(record, (self.transform)(record))
            }
        }
    }

    fn flush(&self) {}
}
