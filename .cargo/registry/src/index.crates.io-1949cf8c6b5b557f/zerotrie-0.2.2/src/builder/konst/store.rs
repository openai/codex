// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This module contains internal collections for the const builder.

use super::super::branch_meta::BranchMeta;

/// A const-friendly slice type. It is backed by a full slice but is primarily intended
/// to represent subslices of the full slice. We need this only because we can't take
/// subslices in const Rust.
#[derive(Debug, Copy, Clone)]
pub(crate) struct ConstSlice<'a, T> {
    /// The full slice.
    full_slice: &'a [T],
    /// The start index of the slice represented by this [`ConstSlice`].
    start: usize,
    /// The non-inclusive end index of the slice represented by this [`ConstSlice`].
    limit: usize,
}

impl<'a, T> ConstSlice<'a, T> {
    /// Creates a [`ConstSlice`] representing an entire slice.
    pub const fn from_slice(other: &'a [T]) -> Self {
        ConstSlice {
            full_slice: other,
            start: 0,
            limit: other.len(),
        }
    }

    /// Creates a [`ConstSlice`] with the given start and limit.
    pub const fn from_manual_slice(full_slice: &'a [T], start: usize, limit: usize) -> Self {
        ConstSlice {
            full_slice,
            start,
            limit,
        }
    }

    /// Returns the length of the [`ConstSlice`].
    pub const fn len(&self) -> usize {
        self.limit - self.start
    }

    /// Gets the element at `index`, panicking if not present.
    pub const fn get_or_panic(&self, index: usize) -> &T {
        &self.full_slice[index + self.start]
    }

    /// Gets the first element or `None` if empty.
    #[cfg(test)]
    pub const fn first(&self) -> Option<&T> {
        if self.len() == 0 {
            None
        } else {
            Some(self.get_or_panic(0))
        }
    }

    /// Gets the last element or `None` if empty.
    pub const fn last(&self) -> Option<&T> {
        if self.len() == 0 {
            None
        } else {
            Some(self.get_or_panic(self.len() - 1))
        }
    }

    /// Gets a subslice of this slice.
    #[cfg(test)]
    pub const fn get_subslice_or_panic(
        &self,
        new_start: usize,
        new_limit: usize,
    ) -> ConstSlice<'a, T> {
        assert!(new_start <= new_limit);
        assert!(new_limit <= self.len());
        ConstSlice {
            full_slice: self.full_slice,
            start: self.start + new_start,
            limit: self.start + new_limit,
        }
    }

    /// Non-const function that returns this [`ConstSlice`] as a regular slice.
    #[cfg(any(test, feature = "alloc"))]
    pub fn as_slice(&self) -> &'a [T] {
        &self.full_slice[self.start..self.limit]
    }
}

impl<'a, T> From<&'a [T]> for ConstSlice<'a, T> {
    fn from(other: &'a [T]) -> Self {
        Self::from_slice(other)
    }
}

/// A const-friendly mutable data structure backed by an array.
#[derive(Debug, Copy, Clone)]
pub(crate) struct ConstArrayBuilder<const N: usize, T> {
    full_array: [T; N],
    start: usize,
    limit: usize,
}

impl<const N: usize, T: Default> Default for ConstArrayBuilder<N, T> {
    fn default() -> Self {
        Self::new_empty([(); N].map(|_| Default::default()), 0)
    }
}

impl<const N: usize, T> ConstArrayBuilder<N, T> {
    /// Creates a new, empty builder of the given size. `cursor` indicates where in the
    /// array new elements will be inserted first. Since we use a lot of prepend operations,
    /// it is common to set `cursor` to `N`.
    pub const fn new_empty(full_array: [T; N], cursor: usize) -> Self {
        assert!(cursor <= N);
        Self {
            full_array,
            start: cursor,
            limit: cursor,
        }
    }

    /// Creates a new builder with some initial content in `[start, limit)`.
    pub const fn from_manual_slice(full_array: [T; N], start: usize, limit: usize) -> Self {
        assert!(start <= limit);
        assert!(limit <= N);
        Self {
            full_array,
            start,
            limit,
        }
    }

    /// Returns the number of initialized elements in the builder.
    pub const fn len(&self) -> usize {
        self.limit - self.start
    }

    /// Whether there are no initialized elements in the builder.
    #[allow(dead_code)]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the initialized elements as a [`ConstSlice`].
    pub const fn as_const_slice(&self) -> ConstSlice<T> {
        ConstSlice::from_manual_slice(&self.full_array, self.start, self.limit)
    }

    /// Non-const function that returns a slice of the initialized elements.
    #[cfg(any(test, feature = "alloc"))]
    pub fn as_slice(&self) -> &[T] {
        &self.full_array[self.start..self.limit]
    }
}

// Certain functions that involve dropping `T` require that it be `Copy`
impl<const N: usize, T: Copy> ConstArrayBuilder<N, T> {
    /// Takes a fully initialized builder as an array. Panics if the builder is not
    /// fully initialized.
    pub const fn const_build_or_panic(self) -> [T; N] {
        if self.start != 0 || self.limit != N {
            let actual_len = self.limit - self.start;
            const PREFIX: &[u8; 31] = b"Buffer too large. Size needed: ";
            let len_bytes: [u8; PREFIX.len() + crate::helpers::MAX_USIZE_LEN_AS_DIGITS] =
                crate::helpers::const_fmt_int(*PREFIX, actual_len);
            let Ok(len_str) = core::str::from_utf8(&len_bytes) else {
                unreachable!()
            };
            panic!("{}", len_str);
        }
        self.full_array
    }

    /// Prepends an element to the front of the builder, panicking if there is no room.
    pub const fn const_push_front_or_panic(mut self, value: T) -> Self {
        if self.start == 0 {
            panic!("Buffer too small");
        }
        self.start -= 1;
        self.full_array[self.start] = value;
        self
    }

    /// Prepends multiple elements to the front of the builder, panicking if there is no room.
    pub const fn const_extend_front_or_panic(mut self, other: ConstSlice<T>) -> Self {
        if self.start < other.len() {
            panic!("Buffer too small");
        }
        self.start -= other.len();
        let mut i = self.start;
        const_for_each!(other, byte, {
            self.full_array[i] = *byte;
            i += 1;
        });
        self
    }
}

impl<const N: usize> ConstArrayBuilder<N, u8> {
    /// Specialized function that performs `self[index] |= bits`
    pub const fn const_bitor_assign(mut self, index: usize, bits: u8) -> Self {
        self.full_array[self.start + index] |= bits;
        self
    }
}

impl<const N: usize, T: Copy> ConstArrayBuilder<N, T> {
    /// Swaps the elements at positions `i` and `j`.
    #[cfg(feature = "alloc")]
    pub fn swap_or_panic(mut self, i: usize, j: usize) -> Self {
        self.full_array.swap(self.start + i, self.start + j);
        self
    }
}

/// Evaluates a block over each element of a const slice. Takes three arguments:
///
/// 1. Expression that resolves to the [`ConstSlice`].
/// 2. Token that will be assigned the value of the element.
/// 3. Block to evaluate for each element.
macro_rules! const_for_each {
    ($safe_const_slice:expr, $item:tt, $inner:expr) => {{
        let mut i = 0;
        while i < $safe_const_slice.len() {
            let $item = $safe_const_slice.get_or_panic(i);
            $inner;
            i += 1;
        }
    }};
}

pub(crate) use const_for_each;

/// A data structure that holds up to K [`BranchMeta`] items.
///
/// Note: It should be possible to store the required data in the builder buffer itself,
/// which would eliminate the need for this helper struct and the limit it imposes.
pub(crate) struct ConstLengthsStack<const K: usize> {
    data: [Option<BranchMeta>; K],
    idx: usize,
}

impl<const K: usize> core::fmt::Debug for ConstLengthsStack<K> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl<const K: usize> ConstLengthsStack<K> {
    /// Creates a new empty [`ConstLengthsStack`].
    pub const fn new() -> Self {
        Self {
            data: [None; K],
            idx: 0,
        }
    }

    /// Returns whether the stack is empty.
    pub const fn is_empty(&self) -> bool {
        self.idx == 0
    }

    /// Adds a [`BranchMeta`] to the stack, panicking if there is no room.
    #[must_use]
    pub const fn push_or_panic(mut self, meta: BranchMeta) -> Self {
        if self.idx >= K {
            panic!(concat!(
                "AsciiTrie Builder: Need more stack (max ",
                stringify!(K),
                ")"
            ));
        }
        self.data[self.idx] = Some(meta);
        self.idx += 1;
        self
    }

    /// Returns a copy of the [`BranchMeta`] on the top of the stack, panicking if
    /// the stack is empty.
    pub const fn peek_or_panic(&self) -> BranchMeta {
        if self.idx == 0 {
            panic!("AsciiTrie Builder: Attempted to peek from an empty stack");
        }
        self.get_or_panic(0)
    }

    /// Returns a copy of the [`BranchMeta`] at the specified index.
    const fn get_or_panic(&self, index: usize) -> BranchMeta {
        if self.idx <= index {
            panic!("AsciiTrie Builder: Attempted to get too deep in a stack");
        }
        match self.data[self.idx - index - 1] {
            Some(x) => x,
            None => unreachable!(),
        }
    }

    /// Removes many [`BranchMeta`]s from the stack, returning them in a [`ConstArrayBuilder`].
    pub const fn pop_many_or_panic(
        mut self,
        len: usize,
    ) -> (Self, ConstArrayBuilder<256, BranchMeta>) {
        debug_assert!(len <= 256);
        let mut result = ConstArrayBuilder::new_empty([BranchMeta::default(); 256], 256);
        let mut ix = 0;
        loop {
            if ix == len {
                break;
            }
            let i = self.idx - ix - 1;
            result = result.const_push_front_or_panic(match self.data[i] {
                Some(x) => x,
                None => panic!("Not enough items in the ConstLengthsStack"),
            });
            ix += 1;
        }
        self.idx -= len;
        (self, result)
    }

    /// Non-const function that returns the initialized elements as a slice.
    fn as_slice(&self) -> &[Option<BranchMeta>] {
        &self.data[0..self.idx]
    }
}

impl<const K: usize> ConstArrayBuilder<K, BranchMeta> {
    /// Converts this builder-array of [`BranchMeta`] to one of the `ascii` fields.
    pub const fn map_to_ascii_bytes(&self) -> ConstArrayBuilder<K, u8> {
        let mut result = ConstArrayBuilder::new_empty([0; K], K);
        let self_as_slice = self.as_const_slice();
        const_for_each!(self_as_slice, value, {
            result = result.const_push_front_or_panic(value.ascii);
        });
        result
    }
}
