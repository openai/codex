//!
//! allocator-api2 crate.
//!
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc as alloc_crate;

#[cfg(not(feature = "nightly"))]
#[macro_use]
mod stable;

#[cfg(feature = "nightly")]
mod nightly;

#[cfg(not(feature = "nightly"))]
pub use self::stable::*;

#[cfg(feature = "nightly")]
pub use self::nightly::*;
