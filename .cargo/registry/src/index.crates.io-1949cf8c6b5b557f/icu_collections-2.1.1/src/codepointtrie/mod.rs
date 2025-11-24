// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This module provides a data structure for an time-efficient lookup of values
//! associated to code points.
//!
//! It is an implementation of the existing [ICU4C UCPTrie](https://unicode-org.github.io/icu-docs/apidoc/released/icu4c/ucptrie_8h.html)
//! / [ICU4J CodePointTrie](https://unicode-org.github.io/icu-docs/apidoc/dev/icu4j/) API.
//!
//! # Architecture
//!
//! ICU4X [`CodePointTrie`] is designed to provide a read-only view of [`CodePointTrie`] data that is exported
//! from ICU4C. Detailed information about the design of the data structure can be found in the documentation
//! for the [`CodePointTrie`] struct.
//!
//! # Examples
//!
//! ## Querying a `CodePointTrie`
//!
//! ```
//! use icu::collections::codepointtrie::planes;
//! let trie = planes::get_planes_trie();
//!
//! assert_eq!(0, trie.get32(0x41)); // 'A' as u32
//! assert_eq!(0, trie.get32(0x13E0)); // 'Ꮰ' as u32
//! assert_eq!(1, trie.get32(0x10044)); // '𐁄' as u32
//! ```
//!
//! [`ICU4X`]: ../icu/index.html

mod cptrie;
mod error;
mod impl_const;
pub mod planes;

#[cfg(feature = "serde")]
pub mod toml;

#[cfg(feature = "serde")]
mod serde;

pub use cptrie::CodePointMapRange;
pub use cptrie::CodePointMapRangeIterator;
pub use cptrie::CodePointTrie;
pub use cptrie::CodePointTrieHeader;
pub use cptrie::FastCodePointTrie;
pub use cptrie::SmallCodePointTrie;
pub use cptrie::TrieType;
pub use cptrie::TrieValue;
pub use cptrie::Typed;
pub use cptrie::TypedCodePointTrie;
pub use error::Error as CodePointTrieError;
