use core::fmt::Display;
use std::path::{self, Path, PathBuf};

#[doc(hidden)]
pub trait AsDisplay<'a>: Sealed {
    // TODO: convert to generic associated type.
    // https://github.com/dtolnay/thiserror/pull/253
    type Target: Display;

    fn as_display(&'a self) -> Self::Target;
}

impl<'a, T> AsDisplay<'a> for &T
where
    T: Display + 'a,
{
    type Target = &'a T;

    fn as_display(&'a self) -> Self::Target {
        *self
    }
}

impl<'a> AsDisplay<'a> for Path {
    type Target = path::Display<'a>;

    #[inline]
    fn as_display(&'a self) -> Self::Target {
        self.display()
    }
}

impl<'a> AsDisplay<'a> for PathBuf {
    type Target = path::Display<'a>;

    #[inline]
    fn as_display(&'a self) -> Self::Target {
        self.display()
    }
}

#[doc(hidden)]
pub trait Sealed {}
impl<T: Display> Sealed for &T {}
impl Sealed for Path {}
impl Sealed for PathBuf {}
