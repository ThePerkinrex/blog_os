use core::{borrow::Borrow, ops::Deref};

use alloc::{borrow::ToOwned, boxed::Box};
use smallvec::SmallVec;

use itertools::Itertools;

#[derive(Debug)]
pub struct ContainsSlashError;

#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct Path {
    components: [Box<str>],
}

impl Path {
    unsafe fn from_slice_unchecked(slice: &[Box<str>]) -> &Self {
        unsafe { core::mem::transmute(slice) }
    }

    pub fn from_slice(slice: &[Box<str>]) -> Result<&Self, ContainsSlashError> {
        for x in slice {
            if x.contains('/') {
                return Err(ContainsSlashError);
            }
        }
        Ok(unsafe { Self::from_slice_unchecked(slice) })
    }

    pub fn parent(&self) -> Option<&Self> {
        if self.components.len() <= 1 {
            None
        } else {
            let len = self.components.len();
            Some(unsafe { Self::from_slice_unchecked(&self.components[..(len - 1)]) })
        }
    }

    pub fn is_absolute(&self) -> bool {
        !self.components.is_empty() && self.components[0].is_empty()
    }

    pub fn join(&self, other: &Self) -> PathBuf {
        let mut x = self.to_owned();
        x.push(other);
        x
    }

    pub const fn len(&self) -> usize {
        self.components.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.components.is_empty()
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

    pub fn push_component<I: Into<Box<str>>>(
        &mut self,
        component: I,
    ) -> Result<(), ContainsSlashError> {
        let c = component.into();
        if c.is_empty() && !self.components.is_empty() {
            Ok(())
        } else if c.contains('/') {
            Err(ContainsSlashError)
        } else {
            self.components.push(c);
            Ok(())
        }
    }

    pub fn push(&mut self, path: &Path) {
        for (_, c) in path
            .components
            .iter()
            .enumerate()
            .skip_while(|(i, s)| s.is_empty() && *i == 0)
        {
            self.components.push(c.clone());
        }
    }

    pub fn as_path(&self) -> &Path {
        unsafe { Path::from_slice_unchecked(&self.components) }
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
        path.push_component("").unwrap();

        assert_eq!("/", format!("{path}"))
    }

    #[test]
    fn abs_path_fmt() {
        let mut path = PathBuf::new();
        path.push_component("").unwrap();
        path.push_component("a").unwrap();

        assert_eq!("/a", format!("{path}"))
    }

    #[test]
    fn abs_path_2_fmt() {
        let mut path = PathBuf::new();
        path.push_component("").unwrap();
        path.push_component("a").unwrap();
        path.push_component("b").unwrap();

        assert_eq!("/a/b", format!("{path}"))
    }

    #[test]
    fn rel_path_fmt() {
        let mut path = PathBuf::new();
        path.push_component("a").unwrap();

        assert_eq!("a", format!("{path}"))
    }

    #[test]
    fn rel_path_2_fmt() {
        let mut path = PathBuf::new();
        path.push_component("a").unwrap();
        path.push_component("b").unwrap();

        assert_eq!("a/b", format!("{path}"))
    }

    #[test]
    fn slash_in_component() {
        let mut path = PathBuf::new();
        assert!(path.push_component("a/").is_err());
    }

    #[test]
    fn ignore_double_empty() {
        let mut path = PathBuf::new();
        assert!(path.push_component("").is_ok());
        assert!(path.push_component("").is_ok());
        assert_eq!(1, path.len())
    }
}
