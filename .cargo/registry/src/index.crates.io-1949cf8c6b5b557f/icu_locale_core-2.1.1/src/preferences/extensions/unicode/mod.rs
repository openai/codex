// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! A set of unicode extensions which correspond to preferences.
//!
//! The module contains a set structs corresponding to Locale [`unicode`](crate::extensions::unicode)
//! extensions for which ICU4X provides implementations of preferences.
//!
//! The macros in this module provide wrappers for creating preferences based on enums and structs.
//!
//! [`Locale`]: crate::Locale
pub mod errors;
pub mod keywords;
mod macros;
#[doc(inline)]
pub use macros::*;
