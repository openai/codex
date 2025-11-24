//! A [TOML]-compatible datetime type
//!
//! [TOML]: https://github.com/toml-lang/toml

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

mod datetime;

#[cfg(feature = "serde")]
#[cfg(feature = "alloc")]
pub mod de;
#[cfg(feature = "serde")]
#[cfg(feature = "alloc")]
pub mod ser;

pub use crate::datetime::Date;
pub use crate::datetime::Datetime;
pub use crate::datetime::DatetimeParseError;
pub use crate::datetime::Offset;
pub use crate::datetime::Time;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
