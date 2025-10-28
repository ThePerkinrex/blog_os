use core::{borrow::Borrow, ops::Deref};

use alloc::{borrow::ToOwned, boxed::Box};
use smallvec::SmallVec;

use itertools::Itertools;

#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct Path {
    components: [Box<str>],
}

impl Path {
    pub fn from_slice(slice: &[Box<str>]) -> &Self {
        unsafe { core::mem::transmute(slice) }
    }

    pub fn parent(&self) -> Option<&Self> {
        if self.components.is_empty() {
            None
        } else {
            let len = self.components.len();
            Some(Self::from_slice(&self.components[..(len - 1)]))
        }
    }
}

impl ToOwned for Path {
    type Owned = PathBuf;

    fn to_owned(&self) -> Self::Owned {
        PathBuf {
            components: self.components.iter().cloned().collect(),
        }
    }
}

impl core::fmt::Display for Path {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.components.len() == 1 && self.components[0].as_ref() == "" {
            return write!(f, "/");
        }
        for c in Itertools::intersperse(self.components.iter().map(AsRef::as_ref), "/") {
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

impl core::fmt::Debug for Path {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self, f)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathBuf {
    components: SmallVec<[Box<str>; 3]>,
}

impl PathBuf {
    pub const fn new() -> Self {
        Self {
            components: SmallVec::new_const(),
        }
    }

    pub fn push<I: Into<Box<str>>>(&mut self, component: I) {
        self.components.push(component.into());
    }

    pub fn as_path(&self) -> &Path {
        Path::from_slice(self.components.as_slice())
    }
}

impl FromIterator<Box<str>> for PathBuf {
    fn from_iter<T: IntoIterator<Item = Box<str>>>(iter: T) -> Self {
        Self {
            components: iter.into_iter().collect(),
        }
    }
}

impl<'a> FromIterator<&'a str> for PathBuf {
    fn from_iter<T: IntoIterator<Item = &'a str>>(iter: T) -> Self {
        Self {
            components: iter.into_iter().map(From::from).collect(),
        }
    }
}

impl Default for PathBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for PathBuf {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.as_path()
    }
}

impl Borrow<Path> for PathBuf {
    fn borrow(&self) -> &Path {
        self
    }
}

impl core::fmt::Display for PathBuf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self.as_path(), f)
    }
}

impl core::fmt::Debug for PathBuf {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_path(), f)
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;

    use crate::path::PathBuf;

    #[test]
    fn root_path_fmt() {
        let mut path = PathBuf::new();
        path.push("");

        assert_eq!("/", format!("{path}"))
    }

    #[test]
    fn abs_path_fmt() {
        let mut path = PathBuf::new();
        path.push("");
        path.push("a");

        assert_eq!("/a", format!("{path}"))
    }

    #[test]
    fn abs_path_2_fmt() {
        let mut path = PathBuf::new();
        path.push("");
        path.push("a");
        path.push("b");

        assert_eq!("/a/b", format!("{path}"))
    }

    #[test]
    fn rel_path_fmt() {
        let mut path = PathBuf::new();
        path.push("a");

        assert_eq!("a", format!("{path}"))
    }

    #[test]
    fn rel_path_2_fmt() {
        let mut path = PathBuf::new();
        path.push("a");
        path.push("b");

        assert_eq!("a/b", format!("{path}"))
    }
}
