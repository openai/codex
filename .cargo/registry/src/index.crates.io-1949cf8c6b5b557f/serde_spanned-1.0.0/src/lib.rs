//! A [serde]-compatible spanned Value
//!
//! This allows capturing the location, in bytes, for a value in the original parsed document for
//! compatible deserializers.
//!
//! [serde]: https://serde.rs/

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]
#![warn(missing_docs)]
#![warn(clippy::std_instead_of_core)]
#![warn(clippy::std_instead_of_alloc)]
// Makes rustc abort compilation if there are any unsafe blocks in the crate.
// Presence of this annotation is picked up by tools such as cargo-geiger
// and lets them ensure that there is indeed no unsafe code as opposed to
// something they couldn't detect (e.g. unsafe added via macro expansion, etc).
#![forbid(unsafe_code)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

#[cfg(feature = "alloc")]
#[allow(unused_extern_crates)]
extern crate alloc;

mod spanned;
pub use crate::spanned::Spanned;
#[cfg(feature = "serde")]
pub mod de;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
