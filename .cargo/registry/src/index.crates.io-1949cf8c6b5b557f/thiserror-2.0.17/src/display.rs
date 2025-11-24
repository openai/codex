use core::fmt::Display;
#[cfg(feature = "std")]
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
    T: Display + ?Sized + 'a,
{
    type Target = &'a T;

    fn as_display(&'a self) -> Self::Target {
        *self
    }
}

#[cfg(feature = "std")]
impl<'a> AsDisplay<'a> for Path {
    type Target = path::Display<'a>;

    #[inline]
    fn as_display(&'a self) -> Self::Target {
        self.display()
    }
}

#[cfg(feature = "std")]
impl<'a> AsDisplay<'a> for PathBuf {
    type Target = path::Display<'a>;

    #[inline]
    fn as_display(&'a self) -> Self::Target {
        self.display()
    }
}

#[doc(hidden)]
pub trait Sealed {}
impl<T: Display + ?Sized> Sealed for &T {}
#[cfg(feature = "std")]
impl Sealed for Path {}
#[cfg(feature = "std")]
impl Sealed for PathBuf {}

// Add a synthetic second impl of AsDisplay to prevent the "single applicable
// impl" rule from making too weird inference decision based on the single impl
// for &T, which could lead to code that compiles with thiserror's std feature
// off but breaks under feature unification when std is turned on by an
// unrelated crate.
#[cfg(not(feature = "std"))]
mod placeholder {
    use super::{AsDisplay, Sealed};
    use core::fmt::{self, Display};

    #[allow(dead_code)]
    pub struct Placeholder;

    impl<'a> AsDisplay<'a> for Placeholder {
        type Target = Self;

        #[inline]
        fn as_display(&'a self) -> Self::Target {
            Placeholder
        }
    }

    impl Display for Placeholder {
        fn fmt(&self, _formatter: &mut fmt::Formatter) -> fmt::Result {
            unreachable!()
        }
    }

    impl Sealed for Placeholder {}
}
