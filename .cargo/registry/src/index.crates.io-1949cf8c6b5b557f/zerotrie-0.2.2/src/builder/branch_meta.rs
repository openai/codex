// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

/// Intermediate metadata for a branch node under construction.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BranchMeta {
    /// The lead byte for this branch. Formerly it was required to be an ASCII byte, but now
    /// it can be any byte.
    pub ascii: u8,
    /// The size in bytes of the trie data reachable from this branch.
    pub local_length: usize,
    /// The size in bytes of this and all later sibling branches.
    pub cumulative_length: usize,
    /// The number of later sibling branches, including this.
    pub count: usize,
}

impl BranchMeta {
    /// Creates a new empty [`BranchMeta`].
    pub const fn default() -> Self {
        BranchMeta {
            ascii: 0,
            cumulative_length: 0,
            local_length: 0,
            count: 0,
        }
    }
}
