// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This crate provides [`ZeroFrom`], a trait for converting types in a zero-copy way.
//!
//! See the documentation of [`ZeroFrom`] for more details.

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
// The lifetimes here are important for safety and explicitly writing
// them out is good even when redundant
#![allow(clippy::needless_lifetimes)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod macro_impls;
mod zero_from;

#[cfg(feature = "derive")]
pub use zerofrom_derive::ZeroFrom;

pub use crate::zero_from::ZeroFrom;
