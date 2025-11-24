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

//! `HashSet` that does not expose insertion order.

use std::hash::Hash;

use allocative::Allocative;

use crate::unordered_map;
use crate::unordered_map::UnorderedMap;
use crate::Equivalent;
use crate::Hashed;
use crate::StarlarkHashValue;

/// `HashSet` that does not expose insertion order.
#[derive(Clone, Allocative, Debug)]
pub struct UnorderedSet<T> {
    map: UnorderedMap<T, ()>,
}

impl<T> Default for UnorderedSet<T> {
    #[inline]
    fn default() -> UnorderedSet<T> {
        UnorderedSet::new()
    }
}

impl<T> UnorderedSet<T> {
    /// Create a new empty set.
    #[inline]
    pub const fn new() -> UnorderedSet<T> {
        UnorderedSet {
            map: UnorderedMap::new(),
        }
    }

    /// Create a new empty set with the specified capacity.
    #[inline]
    pub fn with_capacity(n: usize) -> UnorderedSet<T> {
        UnorderedSet {
            map: UnorderedMap::with_capacity(n),
        }
    }

    /// Insert a value into the set.
    #[inline]
    pub fn insert(&mut self, k: T) -> bool
    where
        T: Hash + Eq,
    {
        self.map.insert(k, ()).is_none()
    }

    /// Clear the set, removing all values.
    #[inline]
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Is the set empty?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Get the number of elements in the set.
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Does the set contain the specified value?
    #[inline]
    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        Q: Hash + Equivalent<T> + ?Sized,
    {
        self.map.contains_key(value)
    }

    /// Does the set contain the specified value?
    #[inline]
    pub fn contains_hashed<Q>(&self, value: Hashed<&Q>) -> bool
    where
        Q: Equivalent<T> + ?Sized,
    {
        self.map.contains_key_hashed(value)
    }

    /// Lower-level access to the underlying map.
    #[inline]
    pub fn raw_entry_mut(&mut self) -> RawEntryBuilderMut<T> {
        RawEntryBuilderMut {
            entry: self.map.raw_entry_mut(),
        }
    }

    /// This function is private.
    fn iter(&self) -> impl Iterator<Item = &T> {
        self.map.entries_unordered().map(|(k, _)| k)
    }

    /// Get the entries in the set, sorted.
    pub fn entries_sorted(&self) -> Vec<&T>
    where
        T: Ord,
    {
        let mut entries = Vec::from_iter(self.iter());
        entries.sort_unstable();
        entries
    }
}

impl<T: Eq + Hash> PartialEq for UnorderedSet<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.map == other.map
    }
}

impl<T: Eq + Hash> Eq for UnorderedSet<T> {}

impl<T: Eq + Hash> FromIterator<T> for UnorderedSet<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> UnorderedSet<T> {
        UnorderedSet {
            map: UnorderedMap::from_iter(iter.into_iter().map(|v| (v, ()))),
        }
    }
}

/// Builder for [`RawEntryMut`].
pub struct RawEntryBuilderMut<'a, T> {
    entry: unordered_map::RawEntryBuilderMut<'a, T, ()>,
}

impl<'a, T> RawEntryBuilderMut<'a, T> {
    /// Find the entry for a key.
    #[inline]
    pub fn from_entry<Q>(self, entry: &Q) -> RawEntryMut<'a, T>
    where
        Q: Hash + Equivalent<T> + ?Sized,
    {
        let entry = Hashed::new(entry);
        self.from_entry_hashed(entry)
    }

    /// Find the entry for a key.
    #[inline]
    pub fn from_entry_hashed<Q>(self, entry: Hashed<&Q>) -> RawEntryMut<'a, T>
    where
        Q: ?Sized + Equivalent<T>,
    {
        self.from_hash(entry.hash(), |k| entry.key().equivalent(k))
    }

    /// Find the entry by hash and equality function.
    #[inline]
    pub fn from_hash<F>(self, hash: StarlarkHashValue, is_match: F) -> RawEntryMut<'a, T>
    where
        F: for<'b> FnMut(&'b T) -> bool,
    {
        match self.entry.from_hash(hash, is_match) {
            unordered_map::RawEntryMut::Occupied(e) => {
                RawEntryMut::Occupied(RawOccupiedEntryMut { entry: e })
            }
            unordered_map::RawEntryMut::Vacant(e) => {
                RawEntryMut::Vacant(RawVacantEntryMut { entry: e })
            }
        }
    }
}

/// Reference to an occupied entry in a [`UnorderedSet`].
pub struct RawOccupiedEntryMut<'a, T> {
    entry: unordered_map::RawOccupiedEntryMut<'a, T, ()>,
}

/// Reference to a vacant entry in a [`UnorderedSet`].
pub struct RawVacantEntryMut<'a, T> {
    entry: unordered_map::RawVacantEntryMut<'a, T, ()>,
}

/// Reference to an entry in a [`UnorderedSet`].
pub enum RawEntryMut<'a, T> {
    /// Occupied entry.
    Occupied(RawOccupiedEntryMut<'a, T>),
    /// Vacant entry.
    Vacant(RawVacantEntryMut<'a, T>),
}

impl<'a, T> RawOccupiedEntryMut<'a, T> {
    /// Remove the entry.
    #[inline]
    pub fn remove(self) -> T {
        self.entry.remove_entry().0
    }

    /// Replace the entry.
    #[inline]
    pub fn insert(&mut self, value: T) -> T {
        self.entry.insert_key(value)
    }
}

impl<'a, T> RawVacantEntryMut<'a, T> {
    /// Insert an entry to the set. This function computes the hash of the key.
    #[inline]
    pub fn insert(self, value: T)
    where
        T: Hash,
    {
        let value = Hashed::new(value);
        self.insert_hashed(value);
    }

    /// Insert an entry to the set.
    #[inline]
    pub fn insert_hashed(self, value: Hashed<T>)
    where
        T: Hash,
    {
        self.entry.insert_hashed(value, ());
    }
}

#[cfg(test)]
mod tests {
    use crate::unordered_set::UnorderedSet;

    #[test]
    fn test_insert() {
        let mut set = UnorderedSet::new();
        assert!(set.insert(10));
        assert!(!set.insert(10));
        assert!(set.insert(20));
        assert!(!set.insert(20));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_entries_sorted() {
        let mut set = UnorderedSet::new();
        set.insert(20);
        set.insert(10);
        set.insert(30);
        assert_eq!(set.entries_sorted(), vec![&10, &20, &30]);
    }
}
