// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Error types

use core::fmt;
use core::num::NonZeroU32;

#[cfg(feature = "std")] use std::boxed::Box;

/// Error type of random number generators
///
/// In order to be compatible with `std` and `no_std`, this type has two
/// possible implementations: with `std` a boxed `Error` trait object is stored,
/// while with `no_std` we merely store an error code.
pub struct Error {
    #[cfg(feature = "std")]
    inner: Box<dyn std::error::Error + Send + Sync + 'static>,
    #[cfg(not(feature = "std"))]
    code: NonZeroU32,
}

impl Error {
    /// Codes at or above this point can be used by users to define their own
    /// custom errors.
    ///
    /// This has a fixed value of `(1 << 31) + (1 << 30) = 0xC000_0000`,
    /// therefore the number of values available for custom codes is `1 << 30`.
    ///
    /// This is identical to [`getrandom::Error::CUSTOM_START`](https://docs.rs/getrandom/latest/getrandom/struct.Error.html#associatedconstant.CUSTOM_START).
    pub const CUSTOM_START: u32 = (1 << 31) + (1 << 30);
    /// Codes below this point represent OS Errors (i.e. positive i32 values).
    /// Codes at or above this point, but below [`Error::CUSTOM_START`] are
    /// reserved for use by the `rand` and `getrandom` crates.
    ///
    /// This is identical to [`getrandom::Error::INTERNAL_START`](https://docs.rs/getrandom/latest/getrandom/struct.Error.html#associatedconstant.INTERNAL_START).
    pub const INTERNAL_START: u32 = 1 << 31;

    /// Construct from any type supporting `std::error::Error`
    ///
    /// Available only when configured with `std`.
    ///
    /// See also `From<NonZeroU32>`, which is available with and without `std`.
    #[cfg(feature = "std")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "std")))]
    #[inline]
    pub fn new<E>(err: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
    {
        Error { inner: err.into() }
    }

    /// Reference the inner error (`std` only)
    ///
    /// When configured with `std`, this is a trivial operation and never
    /// panics. Without `std`, this method is simply unavailable.
    #[cfg(feature = "std")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "std")))]
    #[inline]
    pub fn inner(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        &*self.inner
    }

    /// Unwrap the inner error (`std` only)
    ///
    /// When configured with `std`, this is a trivial operation and never
    /// panics. Without `std`, this method is simply unavailable.
    #[cfg(feature = "std")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "std")))]
    #[inline]
    pub fn take_inner(self) -> Box<dyn std::error::Error + Send + Sync + 'static> {
        self.inner
    }

    /// Extract the raw OS error code (if this error came from the OS)
    ///
    /// This method is identical to `std::io::Error::raw_os_error()`, except
    /// that it works in `no_std` contexts. If this method returns `None`, the
    /// error value can still be formatted via the `Display` implementation.
    #[inline]
    pub fn raw_os_error(&self) -> Option<i32> {
        #[cfg(feature = "std")]
        {
            if let Some(e) = self.inner.downcast_ref::<std::io::Error>() {
                return e.raw_os_error();
            }
        }
        match self.code() {
            Some(code) if u32::from(code) < Self::INTERNAL_START => Some(u32::from(code) as i32),
            _ => None,
        }
    }

    /// Retrieve the error code, if any.
    ///
    /// If this `Error` was constructed via `From<NonZeroU32>`, then this method
    /// will return this `NonZeroU32` code (for `no_std` this is always the
    /// case). Otherwise, this method will return `None`.
    #[inline]
    pub fn code(&self) -> Option<NonZeroU32> {
        #[cfg(feature = "std")]
        {
            self.inner.downcast_ref::<ErrorCode>().map(|c| c.0)
        }
        #[cfg(not(feature = "std"))]
        {
            Some(self.code)
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[cfg(feature = "std")]
        {
            write!(f, "Error {{ inner: {:?} }}", self.inner)
        }
        #[cfg(all(feature = "getrandom", not(feature = "std")))]
        {
            getrandom::Error::from(self.code).fmt(f)
        }
        #[cfg(not(feature = "getrandom"))]
        {
            write!(f, "Error {{ code: {} }}", self.code)
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[cfg(feature = "std")]
        {
            write!(f, "{}", self.inner)
        }
        #[cfg(all(feature = "getrandom", not(feature = "std")))]
        {
            getrandom::Error::from(self.code).fmt(f)
        }
        #[cfg(not(feature = "getrandom"))]
        {
            write!(f, "error code {}", self.code)
        }
    }
}

impl From<NonZeroU32> for Error {
    #[inline]
    fn from(code: NonZeroU32) -> Self {
        #[cfg(feature = "std")]
        {
            Error {
                inner: Box::new(ErrorCode(code)),
            }
        }
        #[cfg(not(feature = "std"))]
        {
            Error { code }
        }
    }
}

#[cfg(feature = "getrandom")]
impl From<getrandom::Error> for Error {
    #[inline]
    fn from(error: getrandom::Error) -> Self {
        #[cfg(feature = "std")]
        {
            Error {
                inner: Box::new(error),
            }
        }
        #[cfg(not(feature = "std"))]
        {
            Error { code: error.code() }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {
    #[inline]
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

#[cfg(feature = "std")]
impl From<Error> for std::io::Error {
    #[inline]
    fn from(error: Error) -> Self {
        if let Some(code) = error.raw_os_error() {
            std::io::Error::from_raw_os_error(code)
        } else {
            std::io::Error::new(std::io::ErrorKind::Other, error)
        }
    }
}

#[cfg(feature = "std")]
#[derive(Debug, Copy, Clone)]
struct ErrorCode(NonZeroU32);

#[cfg(feature = "std")]
impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "error code {}", self.0)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ErrorCode {}

#[cfg(test)]
mod test {
    #[cfg(feature = "getrandom")]
    #[test]
    fn test_error_codes() {
        // Make sure the values are the same as in `getrandom`.
        assert_eq!(super::Error::CUSTOM_START, getrandom::Error::CUSTOM_START);
        assert_eq!(super::Error::INTERNAL_START, getrandom::Error::INTERNAL_START);
    }
}
