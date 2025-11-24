// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! `tinystr` is a utility crate of the [`ICU4X`] project.
//!
//! It includes [`TinyAsciiStr`], a core API for representing small ASCII-only bounded length strings.
//!
//! It is optimized for operations on strings of size 8 or smaller. When use cases involve comparison
//! and conversion of strings for lowercase/uppercase/titlecase, or checking
//! numeric/alphabetic/alphanumeric, `TinyAsciiStr` is the edge performance library.
//!
//! # Examples
//!
//! ```rust
//! use tinystr::TinyAsciiStr;
//!
//! let s1: TinyAsciiStr<4> = "tEsT".parse().expect("Failed to parse.");
//!
//! assert_eq!(s1, "tEsT");
//! assert_eq!(s1.to_ascii_uppercase(), "TEST");
//! assert_eq!(s1.to_ascii_lowercase(), "test");
//! assert_eq!(s1.to_ascii_titlecase(), "Test");
//! assert!(s1.is_ascii_alphanumeric());
//! assert!(!s1.is_ascii_numeric());
//!
//! let s2 = TinyAsciiStr::<8>::try_from_raw(*b"New York")
//!     .expect("Failed to parse.");
//!
//! assert_eq!(s2, "New York");
//! assert_eq!(s2.to_ascii_uppercase(), "NEW YORK");
//! assert_eq!(s2.to_ascii_lowercase(), "new york");
//! assert_eq!(s2.to_ascii_titlecase(), "New york");
//! assert!(!s2.is_ascii_alphanumeric());
//! ```
//!
//! # Details
//!
//! When strings are of size 8 or smaller, the struct transforms the strings as `u32`/`u64` and uses
//! bitmasking to provide basic string manipulation operations:
//! * `is_ascii_numeric`
//! * `is_ascii_alphabetic`
//! * `is_ascii_alphanumeric`
//! * `to_ascii_lowercase`
//! * `to_ascii_uppercase`
//! * `to_ascii_titlecase`
//! * `PartialEq`
//!
//! `TinyAsciiStr` will fall back to `u8` character manipulation for strings of length greater than 8.

//!
//! [`ICU4X`]: ../icu/index.html

// https://github.com/unicode-org/icu4x/blob/main/documents/process/boilerplate.md#library-annotations
#![cfg_attr(not(any(test, doc)), no_std)]
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::exhaustive_structs,
        clippy::exhaustive_enums,
        clippy::trivially_copy_pass_by_ref,
        missing_debug_implementations,
    )
)]

mod macros;

mod ascii;
mod asciibyte;
mod error;
mod int_ops;
mod unvalidated;

#[cfg(feature = "serde")]
mod serde;

#[cfg(feature = "databake")]
mod databake;

#[cfg(feature = "zerovec")]
mod ule;

#[cfg(any(feature = "serde", feature = "alloc"))]
extern crate alloc;

pub use ascii::TinyAsciiStr;
pub use error::ParseError;
pub use unvalidated::UnvalidatedTinyAsciiStr;

/// These are temporary compatability reexports that will be removed
/// in a future version.
pub type TinyStr4 = TinyAsciiStr<4>;
/// These are temporary compatability reexports that will be removed
/// in a future version.
pub type TinyStr8 = TinyAsciiStr<8>;
/// These are temporary compatability reexports that will be removed
/// in a future version.
pub type TinyStr16 = TinyAsciiStr<16>;

#[test]
fn test_size() {
    assert_eq!(
        core::mem::size_of::<TinyStr4>(),
        core::mem::size_of::<Option<TinyStr4>>()
    );
    assert_eq!(
        core::mem::size_of::<TinyStr8>(),
        core::mem::size_of::<Option<TinyStr8>>()
    );
}
