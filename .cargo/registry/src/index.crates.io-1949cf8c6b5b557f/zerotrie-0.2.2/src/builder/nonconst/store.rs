// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This module contains internal collections for the non-const builder.

use super::super::branch_meta::BranchMeta;
use super::super::konst::ConstArrayBuilder;
use alloc::collections::VecDeque;
use alloc::vec::Vec;

/// A trait applied to a data structure for building a ZeroTrie.
pub(crate) trait TrieBuilderStore {
    /// Create a new empty store.
    fn atbs_new_empty() -> Self;

    /// Return the length in bytes of the store.
    fn atbs_len(&self) -> usize;

    /// Push a byte to the front of the store.
    fn atbs_push_front(&mut self, byte: u8);

    /// Push multiple bytes to the front of the store.
    fn atbs_extend_front(&mut self, other: &[u8]);

    /// Read the store into a `Vec<u8>`.
    fn atbs_to_bytes(&self) -> Vec<u8>;

    /// Perform the operation `self[index] |= bits`
    fn atbs_bitor_assign(&mut self, index: usize, bits: u8);

    /// Swap the adjacent ranges `self[start..mid]` and `self[mid..limit]`.
    fn atbs_swap_ranges(&mut self, start: usize, mid: usize, limit: usize);

    /// Remove and return the first element in the store, or `None` if empty.
    fn atbs_pop_front(&mut self) -> Option<u8>;

    /// Prepend `n` zeros to the front of the store.
    fn atbs_prepend_n_zeros(&mut self, n: usize) {
        let mut i = 0;
        while i < n {
            self.atbs_push_front(0);
            i += 1;
        }
    }
}

impl TrieBuilderStore for VecDeque<u8> {
    fn atbs_new_empty() -> Self {
        VecDeque::new()
    }
    fn atbs_len(&self) -> usize {
        self.len()
    }
    fn atbs_push_front(&mut self, byte: u8) {
        self.push_front(byte);
    }
    fn atbs_extend_front(&mut self, other: &[u8]) {
        // TODO: No extend_front on VecDeque?
        self.reserve(other.len());
        for b in other.iter().rev() {
            self.push_front(*b);
        }
    }
    fn atbs_to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.len());
        let (a, b) = self.as_slices();
        v.extend(a);
        v.extend(b);
        v
    }
    fn atbs_bitor_assign(&mut self, index: usize, bits: u8) {
        self[index] |= bits;
    }
    fn atbs_swap_ranges(&mut self, mut start: usize, mut mid: usize, mut limit: usize) {
        if start > mid || mid > limit {
            panic!("Invalid args to atbs_swap_ranges(): start > mid || mid > limit");
        }
        if limit > self.len() {
            panic!(
                "Invalid args to atbs_swap_ranges(): limit out of range: {limit} > {}",
                self.len()
            );
        }
        // The following algorithm is an in-place swap of two adjacent ranges of potentially
        // different lengths. Would make a good coding interview question.
        loop {
            if start == mid || mid == limit {
                return;
            }
            let len0 = mid - start;
            let len1 = limit - mid;
            let mut i = start;
            let mut j = limit - core::cmp::min(len0, len1);
            while j < limit {
                self.swap(i, j);
                i += 1;
                j += 1;
            }
            if len0 < len1 {
                mid = start + len0;
                limit -= len0;
            } else {
                start += len1;
                mid = limit - len1;
            }
        }
    }
    fn atbs_pop_front(&mut self) -> Option<u8> {
        self.pop_front()
    }
}

/// A data structure that holds any number of [`BranchMeta`] items.
pub(crate) struct NonConstLengthsStack {
    data: Vec<BranchMeta>,
}

impl core::fmt::Debug for NonConstLengthsStack {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl NonConstLengthsStack {
    /// Creates a new empty [`NonConstLengthsStack`].
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Returns whether the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Adds a [`BranchMeta`] to the stack.
    pub fn push(&mut self, meta: BranchMeta) {
        self.data.push(meta);
    }

    /// Returns a copy of the [`BranchMeta`] on the top of the stack, panicking if
    /// the stack is empty.
    pub fn peek_or_panic(&self) -> BranchMeta {
        *self.data.last().unwrap()
    }

    /// Removes many [`BranchMeta`]s from the stack, returning them in a [`ConstArrayBuilder`].
    pub fn pop_many_or_panic(&mut self, len: usize) -> ConstArrayBuilder<256, BranchMeta> {
        debug_assert!(len <= 256);
        let mut result = ConstArrayBuilder::new_empty([BranchMeta::default(); 256], 256);
        let mut ix = 0;
        loop {
            if ix == len {
                break;
            }
            let i = self.data.len() - ix - 1;
            // Won't panic because len <= 256
            result = result.const_push_front_or_panic(match self.data.get(i) {
                Some(x) => *x,
                None => panic!("Not enough items in the ConstLengthsStack"),
            });
            ix += 1;
        }
        self.data.truncate(self.data.len() - len);
        result
    }

    /// Non-const function that returns the initialized elements as a slice.
    fn as_slice(&self) -> &[BranchMeta] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_ranges() {
        let s = b"..abcdefghijkl=";
        let mut s = s.iter().copied().collect::<VecDeque<u8>>();
        s.atbs_swap_ranges(2, 7, 14);
        assert_eq!(s.atbs_to_bytes(), b"..fghijklabcde=");
    }
}
