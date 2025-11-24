/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! `Vec` with elements sorted.

use std::slice;
use std::vec;

use allocative::Allocative;

/// Type which enfoces that its elements are sorted. That's it.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Allocative, Default
)]
pub struct SortedVec<T> {
    vec: Vec<T>,
}

impl<T> SortedVec<T> {
    /// Construct an empty `SortedVec`.
    #[inline]
    pub const fn new() -> SortedVec<T> {
        SortedVec { vec: Vec::new() }
    }

    /// Construct without checking that the elements are sorted.
    #[inline]
    pub fn new_unchecked(vec: Vec<T>) -> SortedVec<T>
    where
        T: Ord,
    {
        debug_assert!(vec.iter().zip(vec.iter().skip(1)).all(|(a, b)| a <= b));
        SortedVec { vec }
    }

    /// Iterate over the elements.
    #[inline]
    pub fn iter(&self) -> slice::Iter<T> {
        self.vec.iter()
    }
}

impl<T: Ord> From<Vec<T>> for SortedVec<T> {
    #[inline]
    fn from(mut vec: Vec<T>) -> Self {
        vec.sort();
        SortedVec { vec }
    }
}

impl<T: Ord> FromIterator<T> for SortedVec<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let vec = Vec::from_iter(iter);
        SortedVec::from(vec)
    }
}

impl<T> IntoIterator for SortedVec<T> {
    type Item = T;
    type IntoIter = vec::IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use crate::sorted_vec::SortedVec;

    /// Test `new_unchecked` panics in debug mode when the elements are not sorted.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn test_new_unchecked() {
        SortedVec::new_unchecked(vec![1, 3, 2]);
    }
}
