use core::marker::PhantomData;

use sval::Value;
use uuid::Uuid;

use crate::multitask::task::TaskId;

#[derive(Value)]
#[sval(transparent)]
pub struct RecordOptionalId(Option<UuidValue>);

impl core::fmt::Display for RecordOptionalId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(UuidValue(uuid)) = self.0 {
            write!(f, "{uuid}")
        } else {
            write!(f, "None")
        }
    }
}

impl From<TaskId> for RecordOptionalId {
    fn from(value: TaskId) -> Self {
        Self(value.map(UuidValue))
    }
}

#[derive(Debug)]
struct UuidValue(Uuid);

impl sval::Value for UuidValue {
    fn stream<'sval, S: sval::Stream<'sval> + ?Sized>(&'sval self, stream: &mut S) -> sval::Result {
        use core::fmt::Write;

        struct FmtWrite<'a, 'sval, S: sval::Stream<'sval> + ?Sized> {
            stream: &'a mut S,
            _data: PhantomData<&'sval ()>,
        }

        impl<'a, 'sval, S: sval::Stream<'sval> + ?Sized> Write for FmtWrite<'a, 'sval, S> {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                self.stream
                    .text_fragment_computed(s)
                    .map_err(|_| core::fmt::Error)
            }
        }

        stream.text_begin(Some(36))?; // 32 hex chars + 4 hyphens
        write!(
            FmtWrite {
                stream,
                _data: PhantomData
            },
            "{}",
            self.0
        )
        .map_err(|_| sval::Error::new())?;
        stream.text_end()?;

        Ok(())
    }
}
