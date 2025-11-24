// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A library that provides ASCII-only string and character types, equivalent to the `char`, `str`
//! and `String` types in the standard library.
//!
//! Please refer to the readme file to learn about the different feature modes of this crate.
//!
//! # Minimum supported Rust version
//!
//! The minimum Rust version for 1.1.\* releases is 1.41.1.
//! Later 1.y.0 releases might require newer Rust versions, but the three most
//! recent stable releases at the time of publishing will always be supported.  
//! For example this means that if the current stable Rust version is 1.70 when
//! ascii 1.2.0 is released, then ascii 1.2.\* will not require a newer
//! Rust version than 1.68.
//!
//! # History
//!
//! This package included the Ascii types that were removed from the Rust standard library by the
//! 2014-12 [reform of the `std::ascii` module](https://github.com/rust-lang/rfcs/pull/486). The
//! API changed significantly since then.

#![cfg_attr(not(feature = "std"), no_std)]
// Clippy lints
#![warn(
    clippy::pedantic,
    clippy::decimal_literal_representation,
    clippy::get_unwrap,
    clippy::indexing_slicing
)]
// Naming conventions sometimes go against this lint
#![allow(clippy::module_name_repetitions)]
// We need to get literal non-asciis for tests
#![allow(clippy::non_ascii_literal)]
// Sometimes it looks better to invert the order, such as when the `else` block is small
#![allow(clippy::if_not_else)]
// Shadowing is common and doesn't affect understanding
// TODO: Consider removing `shadow_unrelated`, as it can show some actual logic errors
#![allow(clippy::shadow_unrelated, clippy::shadow_reuse, clippy::shadow_same)]
// A `if let` / `else` sometimes looks better than using iterator adaptors
#![allow(clippy::option_if_let_else)]
// In tests, we're fine with indexing, since a panic is a failure.
#![cfg_attr(test, allow(clippy::indexing_slicing))]
// for compatibility with methods on char and u8
#![allow(clippy::trivially_copy_pass_by_ref)]
// In preparation for feature `unsafe_block_in_unsafe_fn` (https://github.com/rust-lang/rust/issues/71668)
#![allow(unused_unsafe)]

#[cfg(feature = "alloc")]
#[macro_use]
extern crate alloc;
#[cfg(feature = "std")]
extern crate core;

#[cfg(feature = "serde")]
extern crate serde;

#[cfg(all(test, feature = "serde_test"))]
extern crate serde_test;

mod ascii_char;
mod ascii_str;
#[cfg(feature = "alloc")]
mod ascii_string;
mod free_functions;
#[cfg(feature = "serde")]
mod serialization;

pub use ascii_char::{AsciiChar, ToAsciiChar, ToAsciiCharError};
pub use ascii_str::{AsAsciiStr, AsAsciiStrError, AsMutAsciiStr, AsciiStr};
pub use ascii_str::{Chars, CharsMut, CharsRef};
#[cfg(feature = "alloc")]
pub use ascii_string::{AsciiString, FromAsciiError, IntoAsciiString};
pub use free_functions::{caret_decode, caret_encode};
