// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Custom error type(s) for the parent module.

use displaydoc::Display;

/// A custom error type for [`CodePointTrie`](super::CodePointTrie).
#[derive(Copy, Clone, Display, Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// Could not construct [`CodePointTrie`](super::CodePointTrie) from deserialized values
    #[displaydoc("Could not construct CodePointTrie from deserialized values: {reason}")]
    FromDeserialized {
        /// Reason for inability to deserialize values.
        reason: &'static str,
    },
    /// [`CodePointTrie`](super::CodePointTrie) must be constructed from data vector with at least one element
    #[displaydoc("CodePointTrie must be constructed from data vector with at least one element")]
    EmptyDataVector,
    /// [`CodePointTrie`](super::CodePointTrie) must be constructed from index vector long enough to accommodate fast-path access
    #[displaydoc("CodePointTrie must be constructed from index vector long enough to accommodate fast-path access")]
    IndexTooShortForFastAccess,
    /// [`CodePointTrie`](super::CodePointTrie) must be constructed from data vector long enough to accommodate fast-path access
    #[displaydoc("CodePointTrie must be constructed from data vector long enough to accommodate fast-path access")]
    DataTooShortForFastAccess,
}

impl core::error::Error for Error {}
