// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! A set of extensions which correspond to preferences.
//!
//! The module provides structures that represent known values for each keyword
//! in Locale [`extensions`](crate::extensions) with semantic meaning.
//!
//! # Syntactic vs Semantic Extension Handling
//!
//! This module ensures that only valid, recognized values are used, providing semantic validation.
//! It would reject invalid values such as `-u-hc-BB` because `BB` is not a known hour cycle. This
//! is ideal for applications that require strict adherence to standardized values and need to
//! prevent invalid or unrecognized data.
//!
//! If you need to construct syntactically valid Locale extensions without semantic validation,
//! allowing any valid key-value pair regardless of recognition, consider using the
//! [`crate::extensions`] module.
//!
//! [`Locale`]: crate::Locale

pub mod unicode;
