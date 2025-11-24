// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! A data structure offering zero-copy storage and retrieval of byte strings, with a focus
//! on the efficient storage of ASCII strings. Strings are mapped to `usize` values.
//!
//! [`ZeroTrie`] does not support mutation because doing so would require recomputing the entire
//! data structure. Instead, it supports conversion to and from [`LiteMap`] and [`BTreeMap`].
//!
//! There are multiple variants of [`ZeroTrie`] optimized for different use cases.
//!
//! # Examples
//!
//! ```
//! use zerotrie::ZeroTrie;
//!
//! let data: &[(&str, usize)] = &[("abc", 11), ("xyz", 22), ("axyb", 33)];
//!
//! let trie: ZeroTrie<Vec<u8>> = data.iter().copied().collect();
//!
//! assert_eq!(trie.get("axyb"), Some(33));
//! assert_eq!(trie.byte_len(), 18);
//! ```
//!
//! # Internal Structure
//!
//! To read about the internal structure of [`ZeroTrie`], build the docs with private modules:
//!
//! ```bash
//! cargo doc --document-private-items --all-features --no-deps --open
//! ```
//!
//! [`LiteMap`]: litemap::LiteMap
//! [`BTreeMap`]: alloc::collections::BTreeMap

// https://github.com/unicode-org/icu4x/blob/main/documents/process/boilerplate.md#library-annotations
#![cfg_attr(not(any(test, doc)), no_std)]
#![cfg_attr(
    not(test),
    deny(
        // TODO(#4034): Enable the rest of these lints.
        // clippy::indexing_slicing,
        // clippy::unwrap_used,
        clippy::expect_used,
        // clippy::panic,
        clippy::exhaustive_structs,
        clippy::exhaustive_enums, clippy::trivially_copy_pass_by_ref,
        missing_debug_implementations,
    )
)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod builder;
mod byte_phf;
pub mod cursor;
mod error;
#[macro_use]
mod helpers;
mod options;
mod reader;
#[cfg(feature = "serde")]
mod serde;
mod varint;
mod zerotrie;

pub use crate::zerotrie::ZeroAsciiIgnoreCaseTrie;
pub use crate::zerotrie::ZeroTrie;
pub use crate::zerotrie::ZeroTrieExtendedCapacity;
pub use crate::zerotrie::ZeroTriePerfectHash;
pub use crate::zerotrie::ZeroTrieSimpleAscii;
pub use error::ZeroTrieBuildError;

#[cfg(feature = "alloc")]
pub use crate::zerotrie::ZeroTrieStringIterator;
#[cfg(feature = "alloc")]
pub use reader::ZeroTrieIterator;

#[doc(hidden)]
pub mod _internal {
    pub use crate::byte_phf::f1;
    pub use crate::byte_phf::f2;
    pub use crate::byte_phf::PerfectByteHashMap;
}
