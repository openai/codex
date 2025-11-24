//! The `Errno` type, which is a minimal wrapper around an error code.
//!
//! We define the error constants as individual `const`s instead of an enum
//! because we may not know about all of the host's error values and we don't
//! want unrecognized values to create undefined behavior.

use crate::backend;
use core::{fmt, result};
#[cfg(feature = "std")]
use std::error;

/// A specialized [`Result`] type for `rustix` APIs.
pub type Result<T> = result::Result<T, Errno>;

pub use backend::io::errno::Errno;

impl Errno {
    /// Shorthand for `std::io::Error::from(self).kind()`.
    #[cfg(feature = "std")]
    #[inline]
    pub fn kind(self) -> std::io::ErrorKind {
        std::io::Error::from(self).kind()
    }
}

impl fmt::Display for Errno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(feature = "std")]
        {
            std::io::Error::from(*self).fmt(f)
        }
        #[cfg(not(feature = "std"))]
        {
            write!(f, "os error {}", self.raw_os_error())
        }
    }
}

impl fmt::Debug for Errno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(feature = "std")]
        {
            std::io::Error::from(*self).fmt(f)
        }
        #[cfg(not(feature = "std"))]
        {
            write!(f, "os error {}", self.raw_os_error())
        }
    }
}

#[cfg(feature = "std")]
impl error::Error for Errno {}

#[cfg(feature = "std")]
impl From<Errno> for std::io::Error {
    #[inline]
    fn from(err: Errno) -> Self {
        Self::from_raw_os_error(err.raw_os_error() as _)
    }
}

/// Call `f` until it either succeeds or fails other than [`Errno::INTR`].
#[inline]
pub fn retry_on_intr<T, F: FnMut() -> Result<T>>(mut f: F) -> Result<T> {
    loop {
        match f() {
            Err(Errno::INTR) => (),
            result => return result,
        }
    }
}
