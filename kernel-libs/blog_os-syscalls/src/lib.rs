#![no_std]

use core::num::TryFromIntError;

use num_enum::TryFromPrimitiveError;
pub use num_enum::{FromPrimitive, IntoPrimitive, TryFromPrimitive};

macro_rules! enum_with_max {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident : $repr:ty {
            $(
                $(#[$vmeta:meta])*
                $variant:ident $(= $value:expr)?
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[repr($repr)]
        $vis enum $name {
            $(
                $(#[$vmeta])*
                $variant $(= $value)?,
            )*
        }

        impl $name {
            pub const MAX_PRIMITIVE: $repr = {
                let mut max = 0 as $repr;
                $(
                    let val = $name::$variant as $repr;
                    if val > max { max = val; }
                )*
                max
            };


            pub const COUNT: $repr = {
                let mut count = 0 as $repr;
                $(
                    let _ = $name::$variant as $repr;
                    count += 1;
                )*
                count
            };
        }
    };
}

enum_with_max! {
    #[derive(IntoPrimitive, TryFromPrimitive, Debug, Default)]
    #[allow(non_camel_case_types)]
    pub enum SyscallNumber : usize {
        #[default]
        NOP = 0,
        EXIT,
        WRITE,
        BRK,
        YIELD,
        OPEN,
        READ,
        CLOSE,
        FLUSH,
        INIT_DRIVER,
        DELETE_DRIVER,
        STAT,
        NEXT_DIRENTRY
    }
}

#[derive(Debug)]
pub enum SyscallNumError {
    FromInt(TryFromIntError),
    FromPrimitive(TryFromPrimitiveError<SyscallNumber>),
}

impl From<TryFromIntError> for SyscallNumError {
    fn from(value: TryFromIntError) -> Self {
        Self::FromInt(value)
    }
}

impl From<TryFromPrimitiveError<SyscallNumber>> for SyscallNumError {
    fn from(value: TryFromPrimitiveError<SyscallNumber>) -> Self {
        Self::FromPrimitive(value)
    }
}

impl TryFrom<u64> for SyscallNumber {
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Ok(Self::try_from(usize::try_from(value)?)?)
    }

    type Error = SyscallNumError;
}

impl From<SyscallNumber> for u64 {
    fn from(value: SyscallNumber) -> Self {
        Self::try_from(usize::from(value)).unwrap_or_default()
    }
}
