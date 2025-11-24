#[cfg(not(feature = "std"))]
use core::error::Error;
#[cfg(feature = "std")]
use std::error::Error;

use core::panic::UnwindSafe;

pub trait AsDynError<'a>: Sealed {
    fn as_dyn_error(&self) -> &(dyn Error + 'a);
}

impl<'a, T: Error + 'a> AsDynError<'a> for T {
    #[inline]
    fn as_dyn_error(&self) -> &(dyn Error + 'a) {
        self
    }
}

impl<'a> AsDynError<'a> for dyn Error + 'a {
    #[inline]
    fn as_dyn_error(&self) -> &(dyn Error + 'a) {
        self
    }
}

impl<'a> AsDynError<'a> for dyn Error + Send + 'a {
    #[inline]
    fn as_dyn_error(&self) -> &(dyn Error + 'a) {
        self
    }
}

impl<'a> AsDynError<'a> for dyn Error + Send + Sync + 'a {
    #[inline]
    fn as_dyn_error(&self) -> &(dyn Error + 'a) {
        self
    }
}

impl<'a> AsDynError<'a> for dyn Error + Send + Sync + UnwindSafe + 'a {
    #[inline]
    fn as_dyn_error(&self) -> &(dyn Error + 'a) {
        self
    }
}

pub trait Sealed {}
impl<'a, T: Error + 'a> Sealed for T {}
impl<'a> Sealed for dyn Error + 'a {}
impl<'a> Sealed for dyn Error + Send + 'a {}
impl<'a> Sealed for dyn Error + Send + Sync + 'a {}
impl<'a> Sealed for dyn Error + Send + Sync + UnwindSafe + 'a {}
