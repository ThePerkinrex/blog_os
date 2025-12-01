#![no_std]

use num_enum::{IntoPrimitive, TryFromPrimitive};
use thiserror::Error;

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
    #[derive(IntoPrimitive, TryFromPrimitive, Debug, Error)]
    pub enum IOError : u64 {
        #[error("Not found")]
        NotFound = 1,
        #[error("Operation not permitted")]
        OperationNotPermitted,
        #[error("Already exists")]
        AlreadyExists,
        #[error("End of file")]
        EOF,
        #[error("Load elf error")]
        LoadError
    }
}
