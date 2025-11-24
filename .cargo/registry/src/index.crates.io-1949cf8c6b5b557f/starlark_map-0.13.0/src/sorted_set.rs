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

//! `SmallSet` which asserts that its elements are sorted.

use std::hash::Hash;

use allocative::Allocative;

use crate::ordered_set::OrderedSet;
use crate::small_set;
use crate::small_set::SmallSet;
use crate::sorted_vec::SortedVec;
use crate::Equivalent;

/// An immutable `SmallSet` with values guaranteed to be sorted.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Allocative)]
pub struct SortedSet<T: Eq + Hash> {
    inner: OrderedSet<T>,
}

impl<T> SortedSet<T>
where
    T: Eq + Ord + Hash,
{
    /// Construct an empty `SortedSet`.
    #[inline]
    pub const fn new() -> SortedSet<T> {
        SortedSet {
            inner: OrderedSet::new(),
        }
    }

    /// Construct without checking that the elements are sorted.
    #[inline]
    pub fn new_unchecked(inner: OrderedSet<T>) -> Self {
        Self { inner }
    }

    /// Return the number of elements in the set.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the set is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Iterate over the elements.
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &T> {
        self.inner.iter()
    }

    /// Get the element in the set.
    #[inline]
    pub fn get<Q: ?Sized>(&self, value: &Q) -> Option<&T>
    where
        Q: Hash + Equivalent<T>,
    {
        self.inner.get(value)
    }

    /// Check if the set contains the given value.
    #[inline]
    pub fn contains<Q: ?Sized>(&self, value: &Q) -> bool
    where
        Q: Hash + Equivalent<T>,
    {
        self.inner.contains(value)
    }

    /// Get the element at the given index.
    #[inline]
    pub fn get_index(&self, index: usize) -> Option<&T> {
        self.inner.get_index(index)
    }

    /// Iterate over the union of two sets.
    #[inline]
    pub fn union<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = &'a T> {
        // TODO(nga): return sorted.
        self.inner.union(&other.inner)
    }
}

impl<T> From<OrderedSet<T>> for SortedSet<T>
where
    T: Eq + Ord + Hash,
{
    #[inline]
    fn from(mut inner: OrderedSet<T>) -> SortedSet<T> {
        inner.sort();
        SortedSet { inner }
    }
}

impl<T> From<SmallSet<T>> for SortedSet<T>
where
    T: Eq + Ord + Hash,
{
    #[inline]
    fn from(inner: SmallSet<T>) -> SortedSet<T> {
        SortedSet::from(OrderedSet::from(inner))
    }
}

impl<T> From<SortedVec<T>> for SortedSet<T>
where
    T: Eq + Ord + Hash,
{
    #[inline]
    fn from(inner: SortedVec<T>) -> SortedSet<T> {
        SortedSet {
            inner: OrderedSet::from_iter(inner),
        }
    }
}

impl<T> FromIterator<T> for SortedSet<T>
where
    T: Eq + Ord + Hash,
{
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut inner = OrderedSet::from_iter(iter);
        inner.sort();
        SortedSet { inner }
    }
}

impl<T> IntoIterator for SortedSet<T>
where
    T: Eq + Ord + Hash,
{
    type Item = T;
    type IntoIter = small_set::IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a SortedSet<T>
where
    T: Eq + Ord + Hash,
{
    type Item = &'a T;
    type IntoIter = small_set::Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

impl<T> Default for SortedSet<T>
where
    T: Eq + Ord + Hash,
{
    #[inline]
    fn default() -> Self {
        SortedSet::new()
    }
}
