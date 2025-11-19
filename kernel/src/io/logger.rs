use blog_os_log::Logger;
use log::Record;

pub mod data;
pub mod structured;

use crate::{
    _println,
    io::{logger::data::RecordOptionalId, serial::print_json},
    multitask::{get_current_task_id, try_get_current_process_info},
};

macro_rules! print {
    ($($arg:tt)*) => ($crate::io::print(format_args!($($arg)*)));
}

fn print_sink<'a, 'b>(record: &'a log::Record<'b>, data: RecordData) {
    print!(
        "[T{}][P{}][{}][{}] {}\n",
        data.task_id,
        data.process_id,
        record.target(),
        record.level(),
        record.args()
    )
}

#[derive(sval::Value)]
pub struct RecordData {
    pub task_id: RecordOptionalId,
    pub process_id: RecordOptionalId,
}

pub struct ExtendedRecord<'a, 'b> {
    pub record: &'a Record<'b>,
}

fn transform<'a, 'b>(_: &'a log::Record<'b>) -> RecordData {
    RecordData {
        task_id: RecordOptionalId::from(get_current_task_id()),
        process_id: RecordOptionalId::from(try_get_current_process_info().map(|x| x.process_id())),
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
