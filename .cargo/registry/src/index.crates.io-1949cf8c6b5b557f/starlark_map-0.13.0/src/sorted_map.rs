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

//! `SmallMap` which asserts that its elements are sorted.

use std::hash::Hash;

use allocative::Allocative;
use serde::Deserialize;
use serde::Serialize;

use crate::ordered_map::OrderedMap;
use crate::small_map;
use crate::small_map::SmallMap;
use crate::Equivalent;

/// `IndexMap` but with keys sorted.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Allocative)]
pub struct SortedMap<K, V> {
    map: OrderedMap<K, V>,
}

impl<K: Ord + Hash, V> Default for SortedMap<K, V> {
    #[inline]
    fn default() -> Self {
        SortedMap {
            map: OrderedMap::default(),
        }
    }
}

impl<K, V> SortedMap<K, V>
where
    K: Ord + Hash,
{
    /// Construct an empty `SortedMap`.
    #[inline]
    pub const fn new() -> SortedMap<K, V> {
        SortedMap {
            map: OrderedMap::new(),
        }
    }

    /// Iterate over the entries.
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&K, &V)> {
        self.map.iter()
    }

    /// Iterate over the entries, with mutable values.
    #[inline]
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = (&K, &mut V)> {
        self.map.iter_mut()
    }

    /// Iterate over the keys.
    #[inline]
    pub fn keys(&self) -> impl ExactSizeIterator<Item = &K> {
        self.map.keys()
    }

    /// Iterate over the values.
    #[inline]
    pub fn values(&self) -> impl ExactSizeIterator<Item = &V> {
        self.map.values()
    }

    /// Iterate over the values mutably.
    #[inline]
    pub fn values_mut(&mut self) -> impl ExactSizeIterator<Item = &mut V> {
        self.map.values_mut()
    }

    /// Return the number of elements in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check if the map is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Get a reference to the value associated with the given key.
    #[inline]
    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        Q: Hash + Equivalent<K>,
    {
        self.map.get(key)
    }

    /// Get a mutable reference to the value associated with the given key.
    #[inline]
    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        Q: Hash + Equivalent<K>,
    {
        self.map.get_mut(key)
    }

    /// Check if the map contains the given key.
    #[inline]
    pub fn contains_key<Q>(&self, k: &Q) -> bool
    where
        Q: Hash + Equivalent<K> + ?Sized,
    {
        self.map.contains_key(k)
    }

    /// Iterate over the map with hashes.
    #[inline]
    pub fn iter_hashed(&self) -> small_map::IterHashed<K, V> {
        self.map.iter_hashed()
    }
}

impl<K: Ord + Hash, V> FromIterator<(K, V)> for SortedMap<K, V> {
    #[inline]
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let map = OrderedMap::from_iter(iter);
        SortedMap::from(map)
    }
}

impl<K: Ord + Hash, V> From<OrderedMap<K, V>> for SortedMap<K, V> {
    #[inline]
    fn from(mut map: OrderedMap<K, V>) -> SortedMap<K, V> {
        map.sort_keys();
        SortedMap { map }
    }
}

impl<K: Ord + Hash, V> From<SmallMap<K, V>> for SortedMap<K, V> {
    #[inline]
    fn from(map: SmallMap<K, V>) -> SortedMap<K, V> {
        // `OrderedMap: From<SmallMap>` is trivial, so this does not do any extra work
        SortedMap::from(OrderedMap::from(map))
    }
}

impl<K: Ord + Hash, V> IntoIterator for SortedMap<K, V> {
    type Item = (K, V);
    type IntoIter = small_map::IntoIter<K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}

impl<'a, K: Ord + Hash, V> IntoIterator for &'a SortedMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = small_map::Iter<'a, K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<'a, K: Ord + Hash, V> IntoIterator for &'a mut SortedMap<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = small_map::IterMut<'a, K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter_mut()
    }
}

impl<K: Serialize, V: Serialize> Serialize for SortedMap<K, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.map.serialize(serializer)
    }
}

impl<'de, K, V> Deserialize<'de> for SortedMap<K, V>
where
    K: Deserialize<'de> + Hash + Eq,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self {
            map: OrderedMap::deserialize(deserializer)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::sorted_map::SortedMap;

    #[test]
    fn test_from_iter() {
        let map = SortedMap::from_iter([(1, 2), (5, 6), (3, 4)]);
        assert_eq!(
            vec![(&1, &2), (&3, &4), (&5, &6)],
            map.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_value_modification() {
        let mut map = SortedMap::from_iter([(1, vec![1, 2, 3]), (2, vec![4]), (3, vec![5])]);
        let mut keys = map.keys().collect::<Vec<_>>();
        keys.sort();
        assert_eq!(keys, map.keys().collect::<Vec<_>>(),);
        // Support insertion for existing keys
        map.get_mut(&1).unwrap().push(11);
        map.get_mut(&2).unwrap().push(22);
        map.get_mut(&3).unwrap().push(33);

        assert_eq!(
            vec![
                (&1, &vec![1, 2, 3, 11]),
                (&2, &vec![4, 22]),
                (&3, &vec![5, 33])
            ],
            map.iter().collect::<Vec<_>>()
        );
        let mut keys = map.keys().collect::<Vec<_>>();
        keys.sort();
        assert_eq!(map.keys().collect::<Vec<_>>(), keys,);
    }
}
