use core::{borrow::Borrow, ops::Deref};

use alloc::{borrow::ToOwned, boxed::Box};
use kernel_utils::try_from_iterator::TryFromIterator;
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

impl<B: Into<Box<str>>> TryFromIterator<B> for PathBuf {
    type Error = ContainsSlashError;

    fn try_from_iter<T: IntoIterator<Item = B>>(iter: T) -> Result<Self, Self::Error> {
        Ok(Self {
            components: iter
                .into_iter()
                .map(|x| {
                    let x = x.into();
                    if x.contains('/') {
                        Err(ContainsSlashError)
                    } else {
                        Ok(x)
                    }
                })
                .collect::<Result<SmallVec<_>, ContainsSlashError>>()?,
        })
    }
}

// impl<B: Into<Box<str>>> TryFromIterator<B> for PathBuf {
//     type Error = ContainsSlashError;
//     fn try_from_iter<T: IntoIterator<Item = &'a str>>(iter: T) -> Result<Self, Self::Error> {
//         Ok(Self {
//             components: iter
//                 .into_iter()
//                 .map(Box::from)
//                 .map(|x: Box<str>| {
//                     if x.contains('/') {
//                         Err(ContainsSlashError)
//                     } else {
//                         Ok(x)
//                     }
//                 })
//                 .collect::<Result<SmallVec<_>, ContainsSlashError>>()?,
//         })
//     }
// }

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
    use alloc::{boxed::Box, format};
    use kernel_utils::try_from_iterator::TryFromIterator;

    use crate::path::{Path, PathBuf};

    #[test]
    fn root_path_fmt() {
        let mut path = PathBuf::new();
        path.push_component("").unwrap();

        assert_eq!("/", format!("{path:?}"))
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

    #[test]
    fn push_another() {
        let mut path1 = PathBuf::new();
        assert!(path1.push_component("").is_ok());
        assert!(path1.push_component("a").is_ok());
        let mut path2 = PathBuf::new();
        assert!(path2.push_component("b").is_ok());
        path1.push(&path2);
        assert_eq!(3, path1.len());
        assert_eq!("/a/b", format!("{path1}"));
    }

    #[test]
    fn push_another_abs() {
        let mut path1 = PathBuf::new();
        assert!(path1.push_component("").is_ok());
        assert!(path1.push_component("a").is_ok());
        let mut path2 = PathBuf::new();
        assert!(path2.push_component("").is_ok());
        assert!(path2.push_component("b").is_ok());
        path1.push(&path2);
        assert_eq!(3, path1.len());
        assert_eq!("/a/b", format!("{path1}"));
    }

    #[test]
    fn join_abs() {
        let mut path1 = PathBuf::new();
        assert!(path1.push_component("").is_ok());
        assert!(path1.push_component("a").is_ok());
        let mut path2 = PathBuf::new();
        assert!(path2.push_component("").is_ok());
        assert!(path2.push_component("b").is_ok());
        let path3 = path1.join(&path2);
        assert_eq!(3, path3.len());
        assert_eq!("/a/b", format!("{path3}"));
        assert_eq!(2, path1.len());
        assert_eq!("/a", format!("{path1}"));
        assert_eq!(2, path2.len());
        assert_eq!("/b", format!("{path2}"));
    }

    #[test]
    fn not_absolute() {
        let mut path = PathBuf::new();
        path.push_component("a").unwrap();

        assert!(!path.is_absolute())
    }

    #[test]
    fn absolute() {
        let mut path = PathBuf::new();
        path.push_component("").unwrap();
        path.push_component("a").unwrap();

        assert!(path.is_absolute())
    }

    #[test]
    fn not_absolute_empty() {
        let path = PathBuf::new();

        assert!(!path.is_absolute())
    }

    #[test]
    fn absolute_root() {
        let mut path = PathBuf::new();
        path.push_component("").unwrap();

        assert!(path.is_absolute())
    }

    #[test]
    fn parent1() {
        let mut path = PathBuf::new();
        path.push_component("").unwrap();
        path.push_component("a").unwrap();

        assert_eq!(2, path.len());
        assert_eq!(1, path.parent().expect("parent is root").len());
    }

    #[test]
    fn no_parent_abs() {
        let mut path = PathBuf::new();
        path.push_component("").unwrap();

        assert_eq!(1, path.len());
        assert!(path.parent().is_none());
    }

    #[test]
    fn no_parent_rel() {
        let mut path = PathBuf::new();
        path.push_component("a").unwrap();

        assert_eq!(1, path.len());
        assert!(path.parent().is_none());
    }

    #[test]
    fn slash_in_slice() {
        let slice = [Box::from(""), Box::from("a/")];
        assert!(Path::from_slice(&slice).is_err());
    }

    #[test]
    fn no_slash_in_slice() {
        let slice = [Box::from(""), Box::from("a")];
        let path = Path::from_slice(&slice).expect("Correct path");
        assert_eq!("/a", format!("{path}"));
    }

    #[test]
    fn not_empty() {
        let mut path = PathBuf::new();
        path.push_component("a").unwrap();

        assert!(!path.is_empty())
    }

    #[test]
    fn empty() {
        let path = PathBuf::new();

        assert!(path.is_empty())
    }

    #[test]
    fn default() {
        let path = PathBuf::default();

        assert!(path.is_empty())
    }

    #[test]
    fn slash_in_iter() {
        let slice: [Box<str>; 2] = [Box::from(""), Box::from("a/")];
        assert!(PathBuf::try_from_iter(slice).is_err());
    }

    #[test]
    fn no_slash_in_iter() {
        let slice = [Box::from(""), Box::from("a")];
        let path = PathBuf::try_from_iter(slice).expect("Correct path");
        assert_eq!("/a", format!("{path}"));
    }
}
