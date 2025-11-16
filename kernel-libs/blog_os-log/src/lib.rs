#![no_std]

pub struct Logger<const N: usize> {
    pub sinks: [fn(&log::Record); N],
}

impl<const N: usize> log::Log for Logger<N> {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            for sink in &self.sinks {
                (sink)(record)
            }
        }
    }

    fn flush(&self) {}
}
