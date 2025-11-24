// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Definitions of [Unicode Properties] and APIs for
//! retrieving property data in an appropriate data structure.
//!
//! This module is published as its own crate ([`icu_properties`](https://docs.rs/icu_properties/latest/icu_properties/))
//! and as part of the [`icu`](https://docs.rs/icu/latest/icu/) crate. See the latter for more details on the ICU4X project.
//!
//! APIs that return a [`CodePointSetData`] exist for binary properties and certain enumerated
//! properties.
//!
//! APIs that return a [`CodePointMapData`] exist for certain enumerated properties.
//!
//! # Examples
//!
//! ## Property data as `CodePointSetData`s
//!
//! ```
//! use icu::properties::{CodePointSetData, CodePointMapData};
//! use icu::properties::props::{GeneralCategory, Emoji};
//!
//! // A binary property as a `CodePointSetData`
//!
//! assert!(CodePointSetData::new::<Emoji>().contains('🎃')); // U+1F383 JACK-O-LANTERN
//! assert!(!CodePointSetData::new::<Emoji>().contains('木')); // U+6728
//!
//! // An individual enumerated property value as a `CodePointSetData`
//!
//! let line_sep_data = CodePointMapData::<GeneralCategory>::new()
//!     .get_set_for_value(GeneralCategory::LineSeparator);
//! let line_sep = line_sep_data.as_borrowed();
//!
//! assert!(line_sep.contains('\u{2028}'));
//! assert!(!line_sep.contains('\u{2029}'));
//! ```
//!
//! ## Property data as `CodePointMapData`s
//!
//! ```
//! use icu::properties::CodePointMapData;
//! use icu::properties::props::Script;
//!
//! assert_eq!(CodePointMapData::<Script>::new().get('🎃'), Script::Common); // U+1F383 JACK-O-LANTERN
//! assert_eq!(CodePointMapData::<Script>::new().get('木'), Script::Han); // U+6728
//! ```
//!
//! [`ICU4X`]: ../icu/index.html
//! [Unicode Properties]: https://unicode-org.github.io/icu/userguide/strings/properties.html
//! [`CodePointSetData`]: crate::CodePointSetData
//! [`CodePointMapData`]: crate::CodePointMapData

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
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod code_point_set;
pub use code_point_set::{CodePointSetData, CodePointSetDataBorrowed};
mod code_point_map;
pub use code_point_map::{CodePointMapData, CodePointMapDataBorrowed};
mod emoji;
pub use emoji::{EmojiSetData, EmojiSetDataBorrowed};
mod names;
pub use names::{
    PropertyNamesLong, PropertyNamesLongBorrowed, PropertyNamesShort, PropertyNamesShortBorrowed,
    PropertyParser, PropertyParserBorrowed,
};
mod runtime;

// NOTE: The Pernosco debugger has special knowledge
// of the `CanonicalCombiningClass` struct inside the `props`
// module. Please do not change the crate-module-qualified
// name of that struct without coordination.
pub mod props;
pub mod provider;
pub mod script;

mod bidi;
mod trievalue;

mod private {
    pub trait Sealed {}
}
