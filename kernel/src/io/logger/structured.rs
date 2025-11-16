use core::fmt::{self, Write};
use core::marker::PhantomData;
use log::kv::{Error as KvError, Key, Value as KvValue, VisitSource};
use log::Record;
use sval::{Stream, Value as SvalValue};

pub struct RecordSval<'a> {
    record: &'a Record<'a>,
}

fn stream_text_value<'sval, S: Stream<'sval> + ?Sized>(
    text: &'sval str,
) -> impl FnOnce(&mut S) -> sval::Result {
    |stream| stream_text(stream, text)
}

fn stream_text<'sval, S: Stream<'sval> + ?Sized>(stream: &mut S, text: &'sval str) -> sval::Result {
    stream.text_begin(Some(text.len()))?; // because level is &str
    stream.text_fragment(text)?;
    stream.text_end()?;
    Ok(())
}

fn stream_text_computed<'sval, S: Stream<'sval> + ?Sized>(
    stream: &mut S,
    text: &str,
) -> sval::Result {
    stream.text_begin(Some(text.len()))?; // because level is &str
    stream.text_fragment_computed(text)?;
    stream.text_end()?;
    Ok(())
}

fn stream_text_args<'sval, S: Stream<'sval> + ?Sized>(
    stream: &mut S,
    text: &fmt::Arguments,
) -> sval::Result {
    struct StreamWriter<'a, 'sval, S: Stream<'sval> + ?Sized> {
        stream: &'a mut S,
        _marker: PhantomData<&'sval ()>,
    }

    impl<'a, 'sval, S: Stream<'sval> + ?Sized> core::fmt::Write for StreamWriter<'a, 'sval, S> {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            self.stream
                .text_fragment_computed(s)
                .map_err(|_| core::fmt::Error)?;
            Ok(())
        }
    }

    stream.text_begin(None)?; // because level is &str
    let mut w = StreamWriter {
        stream,
        _marker: PhantomData,
    };
    w.write_fmt(*text).map_err(|_| sval::Error::new())?;
    stream.text_end()?;
    Ok(())
}

fn stream_map_kv<'sval, S: Stream<'sval> + ?Sized>(
    stream: &mut S,
    key: &'sval str,
    value: impl FnOnce(&mut S) -> sval::Result,
) -> sval::Result {
    stream.map_key_begin()?;
    stream_text(stream, key)?;
    stream.map_key_end()?;
    stream.map_value_begin()?;
    (value)(stream)?;
    stream.map_value_end()?;

    Ok(())
}

fn stream_map_kv_computed<'sval, S: Stream<'sval> + ?Sized>(
    stream: &mut S,
    key: &str,
    value: impl FnOnce(&mut S) -> sval::Result,
) -> sval::Result {
    stream.map_key_begin()?;
    stream_text_computed(stream, key)?;
    stream.map_key_end()?;
    stream.map_value_begin()?;
    (value)(stream)?;
    stream.map_value_end()?;

    Ok(())
}

impl<'a> SvalValue for RecordSval<'a>
where
    Self: 'a,
{
    fn stream<'sval, S: Stream<'sval> + ?Sized>(&self, stream: &mut S) -> sval::Result
    where
        'a: 'sval,
    {
        // Start a map/object
        stream.map_begin(None)?;

        // level
        stream_map_kv(
            stream,
            "level",
            stream_text_value(self.record.level().as_str()),
        )?;

        // target
        stream_map_kv(stream, "target", stream_text_value(self.record.target()))?;

        // message
        stream_map_kv(stream, "message", |stream| {
            stream_text_args(stream, self.record.args())
        })?;

        // module_path
        if let Some(path) = self.record.module_path() {
            stream_map_kv(stream, "module_path", stream_text_value(path))?;
        }

        // file
        if let Some(file) = self.record.file() {
            // FIX: Use stream_map_kv
            stream_map_kv(stream, "file", stream_text_value(file))?;
        }

        // line
        if let Some(line) = self.record.line() {
            // FIX: Use stream_map_kv and stream the u64 value
            stream_map_kv(stream, "line", |stream| stream.u64(line as u64))?;
        }

        // Now the kv data under "data" key
        stream_map_kv(stream, "data", |stream| {
            // open a map for data
            stream.map_begin(Some(self.record.key_values().count()))?;

            // Visit each kv pair
            struct KvToSval<'r, 'a, S: Stream<'a> + ?Sized> {
                record: &'r Record<'a>,
                stream: &'r mut S,
            }

            impl<'r, 'a, S: Stream<'a> + ?Sized> VisitSource<'a> for KvToSval<'r, 'a, S> {
                fn visit_pair(&mut self, key: Key<'a>, value: KvValue<'a>) -> Result<(), KvError> {
                    stream_map_kv_computed(self.stream, key.as_str(), |stream| {
                        stream.value_computed(&value)
                    })
                    .map_err(|_| KvError::msg("sval error"))
                }
            }

            let mut visitor = KvToSval {
                record: self.record,
                stream,
            };

            self.record
                .key_values()
                .visit(&mut visitor)
                .map_err(|_| sval::Error::new())?;

            // close data map
            stream.map_end()?;
            Ok(())
        })?;

        // close main object
        stream.map_end()?;

        Ok(())
    }
}
