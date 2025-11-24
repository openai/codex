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

//! `HashMap` that does not expose insertion order.

use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::mem;
use std::ops::Index;

use allocative::Allocative;
use hashbrown::raw::Bucket;
use hashbrown::raw::RawTable;

use crate::Equivalent;
use crate::Hashed;
use crate::StarlarkHashValue;
use crate::StarlarkHasher;

/// Hash map which does not expose any insertion order-specific behavior
/// (except `Debug`).
#[derive(Clone, Allocative)]
pub struct UnorderedMap<K, V>(RawTable<(K, V)>);

impl<K, V> Default for UnorderedMap<K, V> {
    #[inline]
    fn default() -> UnorderedMap<K, V> {
        UnorderedMap::new()
    }
}

impl<K, V> UnorderedMap<K, V> {
    /// Create a new empty map.
    #[inline]
    pub const fn new() -> UnorderedMap<K, V> {
        UnorderedMap(RawTable::new())
    }

    /// Create a new empty map with the specified capacity.
    #[inline]
    pub fn with_capacity(n: usize) -> UnorderedMap<K, V> {
        UnorderedMap(RawTable::with_capacity(n))
    }

    /// Get the number of elements in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Is the map empty?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get a reference to the value associated with the given key.
    #[inline]
    pub fn get<Q>(&self, k: &Q) -> Option<&V>
    where
        Q: Hash + Equivalent<K> + ?Sized,
    {
        let k = Hashed::new(k);
        self.get_hashed(k)
    }

    /// Get a reference to the value associated with the given key.
    #[inline]
    pub fn get_hashed<Q>(&self, key: Hashed<&Q>) -> Option<&V>
    where
        Q: Equivalent<K> + ?Sized,
    {
        let hash = key.hash().promote();
        self.0
            .get(hash, |(next_k, _v)| key.key().equivalent(next_k))
            .map(|(_, v)| v)
    }

    /// Get a mutable reference to the value associated with the given key.
    #[inline]
    pub fn get_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        Q: Hash + Equivalent<K> + ?Sized,
    {
        let hash = StarlarkHashValue::new(k).promote();
        self.0
            .get_mut(hash, |(next_k, _v)| k.equivalent(next_k))
            .map(|(_, v)| v)
    }

    /// Does the map contain the specified key?
    #[inline]
    pub fn contains_key<Q>(&self, k: &Q) -> bool
    where
        Q: Hash + Equivalent<K> + ?Sized,
    {
        self.get(k).is_some()
    }

    /// Does the map contain the specified key?
    #[inline]
    pub fn contains_key_hashed<Q>(&self, key: Hashed<&Q>) -> bool
    where
        Q: Equivalent<K> + ?Sized,
    {
        self.get_hashed(key).is_some()
    }

    /// Insert an entry into the map.
    #[inline]
    pub fn insert(&mut self, k: K, v: V) -> Option<V>
    where
        K: Hash + Eq,
    {
        let k = Hashed::new(k);
        match self.raw_entry_mut().from_key_hashed(k.as_ref()) {
            RawEntryMut::Occupied(mut e) => {
                let old = e.insert(v);
                Some(old)
            }
            RawEntryMut::Vacant(e) => {
                e.insert_hashed(k, v);
                None
            }
        }
    }

    /// Remove an entry from the map.
    #[inline]
    pub fn remove<Q>(&mut self, k: &Q) -> Option<V>
    where
        Q: Hash + Equivalent<K> + ?Sized,
    {
        match self.raw_entry_mut().from_key(k) {
            RawEntryMut::Occupied(e) => Some(e.remove()),
            RawEntryMut::Vacant(_) => None,
        }
    }

    /// Preserve only the elements specified by the predicate.
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        // TODO(nga): update hashbrown and use safe `HashTable` instead of this heavily unsafe code:
        //   https://docs.rs/hashbrown/latest/hashbrown/struct.HashTable.html

        // Unsafe code is copy-paste from `hashbrown` crate:
        // https://github.com/rust-lang/hashbrown/blob/f2e62124cd947b5e2309dd6a24c7e422932aae97/src/map.rs#L923
        unsafe {
            for item in self.0.iter() {
                let (k, v) = item.as_mut();
                if !f(k, v) {
                    self.0.erase(item);
                }
            }
        }
    }

    /// Get an entry in the map for in-place manipulation.
    #[inline]
    pub fn entry(&mut self, k: K) -> Entry<K, V>
    where
        K: Hash + Eq,
    {
        let hash = StarlarkHashValue::new(&k).promote();
        if let Some(bucket) = self.0.find(hash, |(next_k, _v)| k.equivalent(next_k)) {
            Entry::Occupied(OccupiedEntry { _map: self, bucket })
        } else {
            Entry::Vacant(VacantEntry {
                map: self,
                key: k,
                hash,
            })
        }
    }

    /// Lower-level access to the entry API.
    #[inline]
    pub fn raw_entry_mut(&mut self) -> RawEntryBuilderMut<K, V> {
        RawEntryBuilderMut { map: self }
    }

    /// Clear the map, removing all entries.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Entries in the map, in arbitrary order.
    #[inline]
    pub fn entries_unordered(&self) -> impl ExactSizeIterator<Item = (&K, &V)> {
        unsafe { self.0.iter().map(|e| (&e.as_ref().0, &e.as_ref().1)) }
    }

    /// Entries in the map, in arbitrary order.
    #[inline]
    pub fn entries_unordered_mut(&mut self) -> impl ExactSizeIterator<Item = (&K, &mut V)> {
        unsafe { self.0.iter().map(|e| (&e.as_ref().0, &mut e.as_mut().1)) }
    }

    /// Keys in the map, in arbitrary order.
    #[inline]
    pub fn keys_unordered(&self) -> impl ExactSizeIterator<Item = &K> {
        self.entries_unordered().map(|(k, _v)| k)
    }

    /// Values in the map, in arbitrary order.
    #[inline]
    pub fn values_unordered(&self) -> impl ExactSizeIterator<Item = &V> {
        self.entries_unordered().map(|(_k, v)| v)
    }

    /// Values in the map, in arbitrary order.
    #[inline]
    pub fn values_unordered_mut(&mut self) -> impl ExactSizeIterator<Item = &mut V> {
        self.entries_unordered_mut().map(|(_k, v)| v)
    }

    /// Into entries, in arbitrary order.
    #[inline]
    pub(crate) fn into_entries_unordered(self) -> impl ExactSizeIterator<Item = (K, V)> {
        self.0.into_iter()
    }

    /// Get the entries in the map, sorted by key.
    pub fn entries_sorted(&self) -> Vec<(&K, &V)>
    where
        K: Ord,
    {
        let mut entries = Vec::from_iter(self.entries_unordered());
        entries.sort_by(|(k1, _), (k2, _)| k1.cmp(k2));
        entries
    }

    /// Convert into `HashMap`.
    pub fn into_hash_map(self) -> std::collections::HashMap<K, V>
    where
        K: Hash + Eq,
    {
        self.into_entries_unordered().collect()
    }

    /// Apply the function to value.
    pub fn map_values<W>(self, mut f: impl FnMut(V) -> W) -> UnorderedMap<K, W>
    where
        K: Hash + Eq,
    {
        let mut map = UnorderedMap::with_capacity(self.len());
        for (k, v) in self.into_entries_unordered() {
            map.insert(k, f(v));
        }
        map
    }
}

impl<K, V, Q: Equivalent<K> + Hash> Index<&Q> for UnorderedMap<K, V> {
    type Output = V;

    #[inline]
    fn index(&self, k: &Q) -> &V {
        self.get(k).expect("key not found")
    }
}

impl<K: Debug, V: Debug> Debug for UnorderedMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.entries_unordered()).finish()
    }
}

impl<K: Eq + Hash, V: Eq> PartialEq for UnorderedMap<K, V> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .entries_unordered()
                .all(|(k, v)| other.get(k) == Some(v))
    }
}

impl<K: Eq + Hash, V: Eq> Eq for UnorderedMap<K, V> {}

impl<K: Hash, V: Hash> Hash for UnorderedMap<K, V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        let mut sum: u64 = 0;
        for (k, v) in self.entries_unordered() {
            let mut hasher = StarlarkHasher::new();
            (k, v).hash(&mut hasher);
            sum = sum.wrapping_add(hasher.finish());
        }
        sum.hash(state);
    }
}

impl<K: Eq + Hash, V> FromIterator<(K, V)> for UnorderedMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> UnorderedMap<K, V> {
        let iter = iter.into_iter();
        let mut map = UnorderedMap::with_capacity(iter.size_hint().0);
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

/// Reference to an occupied entry in a [`UnorderedMap`].
pub struct OccupiedEntry<'a, K, V> {
    _map: &'a mut UnorderedMap<K, V>,
    bucket: Bucket<(K, V)>,
}

/// Reference to a vacant entry in a [`UnorderedMap`].
pub struct VacantEntry<'a, K, V> {
    map: &'a mut UnorderedMap<K, V>,
    hash: u64,
    key: K,
}

/// Reference to an entry in a [`UnorderedMap`].
pub enum Entry<'a, K, V> {
    /// Occupied entry.
    Occupied(OccupiedEntry<'a, K, V>),
    /// Vacant entry.
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K: Eq + Hash, V> VacantEntry<'a, K, V> {
    /// Insert a value into the map.
    #[inline]
    pub fn insert(self, value: V) {
        self.map.0.insert(self.hash, (self.key, value), |(k, _v)| {
            StarlarkHashValue::new(k).promote()
        });
    }
}

impl<'a, K, V> OccupiedEntry<'a, K, V> {
    /// Remove the entry from the map.
    #[inline]
    pub fn get(&self) -> &V {
        unsafe { &self.bucket.as_ref().1 }
    }

    /// Get a reference to the value associated with the entry.
    #[inline]
    pub fn get_mut(&mut self) -> &mut V {
        unsafe { &mut self.bucket.as_mut().1 }
    }

    /// Replace the value associated with the entry.
    #[inline]
    pub fn insert(&mut self, value: V) -> V {
        mem::replace(self.get_mut(), value)
    }
}

/// Builder for [`RawEntryMut`].
pub struct RawEntryBuilderMut<'a, K, V> {
    map: &'a mut UnorderedMap<K, V>,
}

impl<'a, K, V> RawEntryBuilderMut<'a, K, V> {
    /// Find an entry by key.
    #[inline]
    pub fn from_key<Q>(self, k: &Q) -> RawEntryMut<'a, K, V>
    where
        Q: Hash + Equivalent<K> + ?Sized,
    {
        let k = Hashed::new(k);
        self.from_key_hashed(k)
    }

    /// Find an entry by hashed key.
    #[inline]
    pub fn from_key_hashed<Q>(self, k: Hashed<&Q>) -> RawEntryMut<'a, K, V>
    where
        Q: Equivalent<K> + ?Sized,
    {
        self.from_hash(k.hash(), |next_k| k.key().equivalent(next_k))
    }

    /// Find an entry by hash and equality function.
    #[inline]
    pub fn from_hash<F>(self, hash: StarlarkHashValue, mut is_match: F) -> RawEntryMut<'a, K, V>
    where
        F: for<'b> FnMut(&'b K) -> bool,
    {
        let hash = hash.promote();
        if let Some(bucket) = self.map.0.find(hash, |(next_k, _v)| is_match(next_k)) {
            RawEntryMut::Occupied(RawOccupiedEntryMut {
                map: self.map,
                bucket,
            })
        } else {
            RawEntryMut::Vacant(RawVacantEntryMut { map: self.map })
        }
    }
}

/// Occupied entry.
pub struct RawOccupiedEntryMut<'a, K, V> {
    map: &'a mut UnorderedMap<K, V>,
    bucket: Bucket<(K, V)>,
}

/// Vacant entry.
pub struct RawVacantEntryMut<'a, K, V> {
    map: &'a mut UnorderedMap<K, V>,
}

/// Raw entry.
pub enum RawEntryMut<'a, K, V> {
    /// Occupied entry.
    Occupied(RawOccupiedEntryMut<'a, K, V>),
    /// Vacant entry.
    Vacant(RawVacantEntryMut<'a, K, V>),
}

impl<'a, K, V> RawOccupiedEntryMut<'a, K, V> {
    /// Replace the value associated with the entry.
    #[inline]
    pub fn insert(&mut self, value: V) -> V {
        mem::replace(self.get_mut(), value)
    }

    /// Replace the key associated with the entry.
    #[inline]
    pub fn insert_key(&mut self, key: K) -> K {
        mem::replace(self.key_mut(), key)
    }

    /// Get a reference to the value associated with the entry.
    #[inline]
    pub fn get(&self) -> &V {
        unsafe { &self.bucket.as_ref().1 }
    }

    /// Get a reference to the value associated with the entry.
    #[inline]
    pub fn get_mut(&mut self) -> &mut V {
        unsafe { &mut self.bucket.as_mut().1 }
    }

    /// Get a reference to the key associated with the entry.
    #[inline]
    pub fn key_mut(&mut self) -> &mut K {
        unsafe { &mut self.bucket.as_mut().0 }
    }

    /// Remove the entry, return the value.
    #[inline]
    pub fn remove(self) -> V {
        self.remove_entry().1
    }

    /// Remove the entry, return the key and value.
    #[inline]
    pub fn remove_entry(self) -> (K, V) {
        unsafe { self.map.0.remove(self.bucket).0 }
    }
}

impl<'a, K, V> RawVacantEntryMut<'a, K, V> {
    /// Insert entry.
    ///
    /// Not this function computes the hash of the key.
    #[inline]
    pub fn insert(self, key: K, value: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
    {
        let key = Hashed::new(key);
        self.insert_hashed(key, value)
    }

    /// Insert entry.
    #[inline]
    pub fn insert_hashed(self, key: Hashed<K>, value: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
    {
        let (k, v) =
            self.map
                .0
                .insert_entry(key.hash().promote(), (key.into_key(), value), |(k, _v)| {
                    StarlarkHashValue::new(k).promote()
                });
        (k, v)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;

    use crate::unordered_map::UnorderedMap;

    #[test]
    fn test_hash() {
        let a = UnorderedMap::from_iter([(1, 2), (3, 4)]);
        let b = UnorderedMap::from_iter([(3, 4), (1, 2)]);

        fn hash<H: Hash>(x: &H) -> u64 {
            let mut hasher = DefaultHasher::new();
            x.hash(&mut hasher);
            hasher.finish()
        }

        assert_eq!(hash(&a), hash(&b));
    }

    #[test]
    fn test_eq() {
        let a = UnorderedMap::from_iter([(1, 2), (3, 4)]);
        let b = UnorderedMap::from_iter([(3, 4), (1, 2)]);
        let c = UnorderedMap::from_iter([(1, 2), (9, 10)]);
        let d = UnorderedMap::from_iter([(1, 2), (3, 4), (5, 6)]);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn test_insert_remove() {
        let mut map = UnorderedMap::new();
        assert_eq!(map.insert(1, 2), None);
        assert_eq!(UnorderedMap::from_iter([(1, 2)]), map);
        assert_eq!(map.insert(1, 3), Some(2));
        assert_eq!(UnorderedMap::from_iter([(1, 3)]), map);
        assert_eq!(map.remove(&1), Some(3));
        assert_eq!(UnorderedMap::new(), map);
    }

    #[test]
    fn test_entries_sorted() {
        let mut map = UnorderedMap::new();
        map.insert(1, 2);
        map.insert(5, 6);
        map.insert(3, 4);
        assert_eq!(map.entries_sorted(), vec![(&1, &2), (&3, &4), (&5, &6)]);
    }

    #[test]
    fn test_retain() {
        let mut map = UnorderedMap::new();
        for i in 0..1000 {
            map.insert(format!("key{}", i), format!("value{}", i));
        }

        map.retain(|k, v| {
            v.push('x');
            k.ends_with('0')
        });

        assert_eq!(100, map.len());
        for i in 0..1000 {
            if i % 10 == 0 {
                assert_eq!(format!("value{}x", i), map[&format!("key{}", i)]);
            } else {
                assert!(!map.contains_key(&format!("key{}", i)));
            }
        }
    }
}
