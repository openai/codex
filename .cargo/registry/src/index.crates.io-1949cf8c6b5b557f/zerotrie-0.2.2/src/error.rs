// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use displaydoc::Display;

/// Error types for the `zerotrie` crate.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Display)]
#[non_exhaustive]
pub enum ZeroTrieBuildError {
    /// Non-ASCII data was added to an ASCII-only trie.
    #[displaydoc("Non-ASCII cannot be added to an ASCII-only trie")]
    NonAsciiError,
    /// The trie reached its maximum supported capacity.
    #[displaydoc("Reached maximum capacity of trie")]
    CapacityExceeded,
    /// The builder could not solve the perfect hash function.
    #[displaydoc("Failed to solve the perfect hash function. This is rare! Please report your case to the ICU4X team.")]
    CouldNotSolvePerfectHash,
    /// Mixed-case data was added to a case-insensitive trie.
    #[displaydoc("Mixed-case data added to case-insensitive trie")]
    MixedCase,
}

impl core::error::Error for ZeroTrieBuildError {}
