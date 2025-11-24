//! Not public API. Used as `$crate::__private` by macros.

#[doc(hidden)]
pub use core::marker::{Send, Sized, Sync};
#[doc(hidden)]
pub use serde;

#[doc(hidden)]
pub type Result<T, E> = core::result::Result<T, E>;

#[doc(hidden)]
pub fn require_erased_serialize_impl<T>()
where
    T: ?Sized + crate::Serialize,
{
}
