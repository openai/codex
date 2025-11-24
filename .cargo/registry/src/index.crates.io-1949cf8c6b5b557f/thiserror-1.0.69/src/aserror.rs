use core::panic::UnwindSafe;
use std::error::Error;

#[doc(hidden)]
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

#[doc(hidden)]
pub trait Sealed {}
impl<T: Error> Sealed for T {}
impl<'a> Sealed for dyn Error + 'a {}
impl<'a> Sealed for dyn Error + Send + 'a {}
impl<'a> Sealed for dyn Error + Send + Sync + 'a {}
impl<'a> Sealed for dyn Error + Send + Sync + UnwindSafe + 'a {}
