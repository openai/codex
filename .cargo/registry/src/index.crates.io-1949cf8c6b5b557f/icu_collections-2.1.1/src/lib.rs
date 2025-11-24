// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Efficient collections for Unicode data.
//!
//! This module is published as its own crate ([`icu_collections`](https://docs.rs/icu_collections/latest/icu_collections/))
//! and as part of the [`icu`](https://docs.rs/icu/latest/icu/) crate. See the latter for more details on the ICU4X project.
//!
//! ICU4X [`CodePointTrie`](crate::codepointtrie::CodePointTrie) provides a read-only view of `CodePointTrie` data that is exported
//! from ICU4C. Detailed information about the design of the data structure can be found in the documentation
//! for the [`CodePointTrie`](crate::codepointtrie::CodePointTrie) struct.
//!
//! ICU4X [`CodePointInversionList`](`crate::codepointinvlist::CodePointInversionList`) provides necessary functionality for highly efficient querying of sets of Unicode characters.
//! It is an implementation of the existing [ICU4C UnicodeSet API](https://unicode-org.github.io/icu-docs/apidoc/released/icu4c/classicu_1_1UnicodeSet.html).
//!
//! ICU4X [`Char16Trie`](`crate::char16trie::Char16Trie`) provides a data structure for a space-efficient and time-efficient lookup of
//! sequences of 16-bit units (commonly but not necessarily UTF-16 code units)
//! which map to integer values.
//! It is an implementation of the existing [ICU4C UCharsTrie](https://unicode-org.github.io/icu-docs/apidoc/released/icu4c/classicu_1_1UCharsTrie.html)
//! / [ICU4J CharsTrie](https://unicode-org.github.io/icu-docs/apidoc/released/icu4j/com/ibm/icu/util/CharsTrie.html) API.

// https://github.com/unicode-org/icu4x/blob/main/documents/process/boilerplate.md#library-annotations
#![cfg_attr(not(any(test, doc)), no_std)]
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic
    )
)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod char16trie;
pub mod codepointinvlist;
pub mod codepointinvliststringlist;
pub mod codepointtrie;

pub(crate) mod iterator_utils;
