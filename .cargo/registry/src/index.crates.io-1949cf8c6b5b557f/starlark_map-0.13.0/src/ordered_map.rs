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

//! `SmallMap` which considers iteration order important for equality and hash.

use std::cmp::Ordering;
use std::hash::Hash;

use allocative::Allocative;
use serde::Deserialize;
use serde::Serialize;

use crate::small_map;
use crate::small_map::SmallMap;
use crate::Equivalent;

/// Wrapper for `SmallMap` which considers map equal if iteration order is equal.
#[derive(Debug, Clone, Allocative)]
pub struct OrderedMap<K, V>(SmallMap<K, V>);

impl<K, V> OrderedMap<K, V> {
    /// Create a new empty map.
    #[inline]
    pub const fn new() -> OrderedMap<K, V> {
        OrderedMap(SmallMap::new())
    }

    /// Create a new empty map with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> OrderedMap<K, V> {
        OrderedMap(SmallMap::with_capacity(capacity))
    }

    /// Get the number of elements in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the map is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over the entries.
    #[inline]
    pub fn iter(&self) -> small_map::Iter<K, V> {
        self.0.iter()
    }

    /// Iterate over the entries, with mutable values.
    #[inline]
    pub fn iter_mut(&mut self) -> small_map::IterMut<K, V> {
        self.0.iter_mut()
    }

    /// Iterate over the keys.
    #[inline]
    pub fn keys(&self) -> impl ExactSizeIterator<Item = &K> {
        self.0.keys()
    }

    /// Iterate over the values.
    #[inline]
    pub fn values(&self) -> impl ExactSizeIterator<Item = &V> {
        self.0.values()
    }

    /// Iterate over the values, with mutable values.
    #[inline]
    pub fn values_mut(&mut self) -> impl ExactSizeIterator<Item = &mut V> {
        self.0.values_mut()
    }

    /// Get a reference to the value associated with the given key.
    #[inline]
    pub fn get<'a, Q>(&'a self, k: &Q) -> Option<&'a V>
    where
        Q: Hash + Equivalent<K> + ?Sized,
    {
        self.0.get(k)
    }

    /// Get a mutable reference to the value associated with the given key.
    #[inline]
    pub fn get_mut<'a, Q>(&'a mut self, k: &Q) -> Option<&'a mut V>
    where
        Q: Hash + Equivalent<K> + ?Sized,
    {
        self.0.get_mut(k)
    }

    /// Find an entry by an index.
    #[inline]
    pub fn get_index(&self, index: usize) -> Option<(&K, &V)> {
        self.0.get_index(index)
    }

    /// Find an entry index for a given key.
    #[inline]
    pub fn get_index_of<Q>(&self, key: &Q) -> Option<usize>
    where
        Q: Hash + Equivalent<K> + ?Sized,
    {
        self.0.get_index_of(key)
    }

    /// Check if the map contains the given key.
    #[inline]
    pub fn contains_key<Q>(&self, k: &Q) -> bool
    where
        Q: Hash + Equivalent<K> + ?Sized,
    {
        self.0.contains_key(k)
    }

    /// Insert an entry into the map.
    #[inline]
    pub fn insert(&mut self, k: K, v: V) -> Option<V>
    where
        K: Hash + Eq,
    {
        self.0.insert(k, v)
    }

    /// Remove an entry by key.
    #[inline]
    pub fn remove<Q>(&mut self, k: &Q) -> Option<V>
    where
        Q: Hash + Equivalent<K> + ?Sized,
    {
        self.0.shift_remove(k)
    }

    /// Clear the map.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    /// Get an entry by key.
    #[inline]
    pub fn entry(&mut self, k: K) -> small_map::Entry<'_, K, V>
    where
        K: Hash + Eq,
    {
        self.0.entry(k)
    }

    /// Sort the map by keys.
    #[inline]
    pub fn sort_keys(&mut self)
    where
        K: Ord,
    {
        self.0.sort_keys()
    }

    /// Iterate over the map with hashes.
    #[inline]
    pub fn iter_hashed(&self) -> small_map::IterHashed<K, V> {
        self.0.iter_hashed()
    }
}

impl<K, V> Default for OrderedMap<K, V> {
    #[inline]
    fn default() -> OrderedMap<K, V> {
        OrderedMap(SmallMap::default())
    }
}

impl<K: PartialEq, V: PartialEq> PartialEq for OrderedMap<K, V> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ordered(&other.0)
    }
}

impl<K: Eq, V: Eq> Eq for OrderedMap<K, V> {}

impl<K: Hash, V: Hash> Hash for OrderedMap<K, V> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash_ordered(state)
    }
}

impl<K: PartialOrd, V: PartialOrd> PartialOrd for OrderedMap<K, V> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl<K: Eq + Ord, V: Eq + Ord> Ord for OrderedMap<K, V> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.iter().cmp(other.iter())
    }
}

impl<K, V> From<SmallMap<K, V>> for OrderedMap<K, V> {
    #[inline]
    fn from(map: SmallMap<K, V>) -> OrderedMap<K, V> {
        OrderedMap(map)
    }
}

impl<K, V> FromIterator<(K, V)> for OrderedMap<K, V>
where
    K: Hash + Eq,
{
    #[inline]
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> OrderedMap<K, V> {
        OrderedMap(SmallMap::from_iter(iter))
    }
}

impl<K, V> Extend<(K, V)> for OrderedMap<K, V>
where
    K: Hash + Eq,
{
    #[inline]
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        self.0.extend(iter)
    }
}

impl<K, V> IntoIterator for OrderedMap<K, V> {
    type Item = (K, V);
    type IntoIter = small_map::IntoIter<K, V>;

    #[inline]
    fn into_iter(self) -> small_map::IntoIter<K, V> {
        self.0.into_iter()
    }
}

impl<'a, K, V> IntoIterator for &'a OrderedMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = small_map::Iter<'a, K, V>;

    #[inline]
    fn into_iter(self) -> small_map::Iter<'a, K, V> {
        self.0.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut OrderedMap<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = small_map::IterMut<'a, K, V>;

    #[inline]
    fn into_iter(self) -> small_map::IterMut<'a, K, V> {
        self.0.iter_mut()
    }
}

impl<K: Serialize, V: Serialize> Serialize for OrderedMap<K, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, K, V> Deserialize<'de> for OrderedMap<K, V>
where
    K: Deserialize<'de> + Hash + Eq,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        SmallMap::deserialize(deserializer).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;

    use crate::ordered_map::OrderedMap;

    #[test]
    fn test_keys_are_not_hashed_when_map_is_hashed() {
        struct Tester {
            /// Number of times `hash` was called.
            hash_count: Cell<u32>,
        }

        impl PartialEq for Tester {
            fn eq(&self, _other: &Self) -> bool {
                true
            }
        }

        impl Eq for Tester {}

        impl Hash for Tester {
            fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {
                self.hash_count.set(self.hash_count.get() + 1);
            }
        }

        let map = OrderedMap::from_iter([(
            Tester {
                hash_count: Cell::new(0),
            },
            Tester {
                hash_count: Cell::new(0),
            },
        )]);
        assert_eq!(1, map.keys().next().unwrap().hash_count.get());
        assert_eq!(0, map.values().next().unwrap().hash_count.get());

        let mut hasher = DefaultHasher::new();

        map.hash(&mut hasher);
        assert_eq!(1, map.keys().next().unwrap().hash_count.get());
        assert_eq!(1, map.values().next().unwrap().hash_count.get());

        map.hash(&mut hasher);
        assert_eq!(1, map.keys().next().unwrap().hash_count.get());
        assert_eq!(2, map.values().next().unwrap().hash_count.get());

        map.hash(&mut hasher);
        assert_eq!(1, map.keys().next().unwrap().hash_count.get());
        assert_eq!(3, map.values().next().unwrap().hash_count.get());
    }

    #[test]
    fn test_serde() {
        let map = OrderedMap::from_iter([("a", 1), ("b", 2)]);
        let serialized = serde_json::to_string(&map).unwrap();
        assert_eq!(serialized, "{\"a\":1,\"b\":2}");
        let map2 = serde_json::from_str(&serialized).unwrap();
        assert_eq!(map, map2);
    }
}
