use blog_os_log::Logger;
use log::Record;

pub mod structured;

use crate::{_println, io::serial::print_json, multitask::get_current_task_id};

macro_rules! print {
    ($($arg:tt)*) => ($crate::io::print(format_args!($($arg)*)));
}

fn print_sink<'a, 'b>(record: &'a log::Record<'b>, data: RecordData) {
    print!(
        "[task: {:?}][{}][{}] {}\n",
        data.task_id,
        record.level(),
        record.target(),
        record.args()
    )
}

#[derive(Debug, sval::Value)]
pub struct RecordData {
    pub task_id: Option<uuid::Uuid>,
}

pub struct ExtendedRecord<'a, 'b> {
    pub record: &'a Record<'b>,
}

fn transform<'a, 'b>(_: &'a log::Record<'b>) -> RecordData {
    RecordData {
        task_id: get_current_task_id(),
    }
}

static LOGGER: Logger<2, RecordData> = Logger {
    sinks: [print_sink, print_json],
    transform,
};

pub fn init() {
    _println!("Setting up logger");
    log::set_logger(&LOGGER).expect("the logger");
    log::set_max_level(log::LevelFilter::Trace);
    _println!("Set up logger");
}
