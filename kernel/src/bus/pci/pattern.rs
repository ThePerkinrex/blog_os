use core::{fmt::Display, str::FromStr};

use num_traits::Num;

use crate::bus::pci::class::PciClass;

#[derive(Debug)]
pub struct PatternParseError;

pub struct PciPattern {
    pub vendor: Option<u16>,    // None means any
    pub device: Option<u16>,    // None means any
    pub subvendor: Option<u16>, // None means any
    pub subdevice: Option<u16>, // None means any
    pub class: PciClass,
    pub classmask: PciClass,
}

fn parse_num<N: Num>(s: &str) -> Result<N, N::FromStrRadixErr> {
    #[allow(clippy::option_if_let_else)]
    if let Some(s) = s.strip_prefix("0x") {
        N::from_str_radix(s, 16)
    } else if let Some(s) = s.strip_prefix("0b") {
        N::from_str_radix(s, 2)
    } else if let Some(s) = s.strip_prefix("0o") {
        N::from_str_radix(s, 8)
    } else {
        N::from_str_radix(s, 10)
    }
}

fn parse_or_any<F: FnOnce(&str) -> Result<T, E>, T, E>(s: &str, f: F) -> Result<Option<T>, E> {
    if s == "*" { Ok(None) } else { f(s).map(Some) }
}

impl FromStr for PciPattern {
    type Err = PatternParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split_whitespace();
        let res = Ok(Self {
            vendor: parse_or_any(parts.next().ok_or(PatternParseError)?, parse_num)
                .map_err(|_| PatternParseError)?,
            device: parse_or_any(parts.next().ok_or(PatternParseError)?, parse_num)
                .map_err(|_| PatternParseError)?,
            subvendor: parse_or_any(parts.next().ok_or(PatternParseError)?, parse_num)
                .map_err(|_| PatternParseError)?,
            subdevice: parse_or_any(parts.next().ok_or(PatternParseError)?, parse_num)
                .map_err(|_| PatternParseError)?,
            class: PciClass::from_bits(
                parse_num(parts.next().ok_or(PatternParseError)?).map_err(|_| PatternParseError)?,
            ),
            classmask: PciClass::from_bits(
                parse_num(parts.next().ok_or(PatternParseError)?).map_err(|_| PatternParseError)?,
            ),
        });
        if parts.next().is_none() {
            res
        } else {
            Err(PatternParseError)
        }
    }
}

impl Display for PciPattern {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        fn write_part(f: &mut core::fmt::Formatter<'_>, data: Option<u16>) -> core::fmt::Result {
            if let Some(data) = data {
                write!(f, "0x{data:x}")
            } else {
                write!(f, "*")
            }
        }
        write_part(f, self.vendor)?;
        write!(f, " ")?;
        write_part(f, self.device)?;

        write!(f, " ")?;
        write_part(f, self.subvendor)?;
        write!(f, " ")?;
        write_part(f, self.subdevice)?;
        write!(
            f,
            " 0x{:x} 0x{:x}",
            self.class.into_bits(),
            self.classmask.into_bits()
        )?;
        Ok(())
    }
}
