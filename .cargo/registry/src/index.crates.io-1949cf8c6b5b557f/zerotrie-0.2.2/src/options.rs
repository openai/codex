// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Options for building and reading from a ZeroTrie.
//!
//! These options are internal to the crate. A small selection of options
//! are exported by way of the different public types on this crate.

/// Whether to use the perfect hash function in the ZeroTrie.
#[derive(Copy, Clone)]
pub(crate) enum PhfMode {
    /// Use binary search for all branch nodes.
    BinaryOnly,
    /// Use the perfect hash function for large branch nodes.
    UsePhf,
}

impl PhfMode {
    #[cfg(feature = "serde")]
    const fn to_u8_flag(self) -> u8 {
        match self {
            Self::BinaryOnly => 0,
            Self::UsePhf => 0x1,
        }
    }
}

/// Whether to support non-ASCII data in the ZeroTrie.
#[derive(Copy, Clone)]
pub(crate) enum AsciiMode {
    /// Support only ASCII, returning an error if non-ASCII is found.
    AsciiOnly,
    /// Support all data, creating span nodes for non-ASCII bytes.
    BinarySpans,
}

impl AsciiMode {
    #[cfg(feature = "serde")]
    const fn to_u8_flag(self) -> u8 {
        match self {
            Self::AsciiOnly => 0,
            Self::BinarySpans => 0x2,
        }
    }
}

/// Whether to enforce a limit to the capacity of the ZeroTrie.
#[derive(Copy, Clone)]
pub(crate) enum CapacityMode {
    /// Return an error if the trie requires a branch of more than 2^32 bytes.
    Normal,
    /// Construct the trie without returning an error.
    Extended,
}

impl CapacityMode {
    #[cfg(feature = "serde")]
    const fn to_u8_flag(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Extended => 0x4,
        }
    }
}

/// How to handle strings with mixed ASCII case at a node, such as "abc" and "Abc"
#[derive(Copy, Clone)]
pub(crate) enum CaseSensitivity {
    /// Allow all strings and sort them by byte value.
    Sensitive,
    /// Reject strings with different case and sort them as if `to_ascii_lowercase` is called.
    IgnoreCase,
}

impl CaseSensitivity {
    #[cfg(feature = "serde")]
    const fn to_u8_flag(self) -> u8 {
        match self {
            Self::Sensitive => 0,
            Self::IgnoreCase => 0x8,
        }
    }
}

#[derive(Copy, Clone)]
pub(crate) struct ZeroTrieBuilderOptions {
    pub phf_mode: PhfMode,
    pub ascii_mode: AsciiMode,
    pub capacity_mode: CapacityMode,
    pub case_sensitivity: CaseSensitivity,
}

impl ZeroTrieBuilderOptions {
    #[cfg(feature = "serde")]
    pub(crate) const fn to_u8_flags(self) -> u8 {
        self.phf_mode.to_u8_flag()
            | self.ascii_mode.to_u8_flag()
            | self.capacity_mode.to_u8_flag()
            | self.case_sensitivity.to_u8_flag()
    }
}

pub(crate) trait ZeroTrieWithOptions {
    const OPTIONS: ZeroTrieBuilderOptions;
}

/// All branch nodes are binary search
/// and there are no span nodes.
impl<S: ?Sized> ZeroTrieWithOptions for crate::ZeroTrieSimpleAscii<S> {
    const OPTIONS: ZeroTrieBuilderOptions = ZeroTrieBuilderOptions {
        phf_mode: PhfMode::BinaryOnly,
        ascii_mode: AsciiMode::AsciiOnly,
        capacity_mode: CapacityMode::Normal,
        case_sensitivity: CaseSensitivity::Sensitive,
    };
}

impl<S: ?Sized> crate::ZeroTrieSimpleAscii<S> {
    #[cfg(feature = "serde")]
    pub(crate) const FLAGS: u8 = Self::OPTIONS.to_u8_flags();
}

/// All branch nodes are binary search
/// and nodes use case-insensitive matching.
impl<S: ?Sized> ZeroTrieWithOptions for crate::ZeroAsciiIgnoreCaseTrie<S> {
    const OPTIONS: ZeroTrieBuilderOptions = ZeroTrieBuilderOptions {
        phf_mode: PhfMode::BinaryOnly,
        ascii_mode: AsciiMode::AsciiOnly,
        capacity_mode: CapacityMode::Normal,
        case_sensitivity: CaseSensitivity::IgnoreCase,
    };
}

/// Branch nodes could be either binary search or PHF.
impl<S: ?Sized> ZeroTrieWithOptions for crate::ZeroTriePerfectHash<S> {
    const OPTIONS: ZeroTrieBuilderOptions = ZeroTrieBuilderOptions {
        phf_mode: PhfMode::UsePhf,
        ascii_mode: AsciiMode::BinarySpans,
        capacity_mode: CapacityMode::Normal,
        case_sensitivity: CaseSensitivity::Sensitive,
    };
}

/// No limited capacity assertion.
impl<S: ?Sized> ZeroTrieWithOptions for crate::ZeroTrieExtendedCapacity<S> {
    const OPTIONS: ZeroTrieBuilderOptions = ZeroTrieBuilderOptions {
        phf_mode: PhfMode::UsePhf,
        ascii_mode: AsciiMode::BinarySpans,
        capacity_mode: CapacityMode::Extended,
        case_sensitivity: CaseSensitivity::Sensitive,
    };
}
