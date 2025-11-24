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

mod hint;
mod iter;
mod simd;

use std::hash::Hash;
use std::hash::Hasher;
use std::mem;

use allocative::Allocative;
use equivalent::Equivalent;

use crate::hash_value::StarlarkHashValue;
use crate::hashed::Hashed;
pub(crate) use crate::vec2::Vec2;
use crate::vec_map::hint::likely;
pub(crate) use crate::vec_map::iter::IntoIter;
pub(crate) use crate::vec_map::iter::IntoIterHashed;
pub(crate) use crate::vec_map::iter::Iter;
pub(crate) use crate::vec_map::iter::IterHashed;
pub(crate) use crate::vec_map::iter::IterMut;
pub(crate) use crate::vec_map::iter::IterMutUnchecked;
pub(crate) use crate::vec_map::iter::Keys;
pub(crate) use crate::vec_map::iter::Values;
pub(crate) use crate::vec_map::iter::ValuesMut;
use crate::vec_map::simd::find_hash_in_array;

#[derive(Debug, Clone, Allocative)]
pub(crate) struct VecMap<K, V> {
    buckets: Vec2<(K, V), StarlarkHashValue>,
}

impl<K, V> Default for VecMap<K, V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> VecMap<K, V> {
    #[inline]
    pub(crate) const fn new() -> Self {
        VecMap {
            buckets: Vec2::new(),
        }
    }

    #[inline]
    pub(crate) fn with_capacity(n: usize) -> Self {
        VecMap {
            buckets: Vec2::with_capacity(n),
        }
    }

    pub(crate) fn reserve(&mut self, additional: usize) {
        self.buckets.reserve(additional);
    }

    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        self.buckets.capacity()
    }

    #[inline]
    pub(crate) fn get_index_of_hashed_raw(
        &self,
        hash: StarlarkHashValue,
        mut eq: impl FnMut(&K) -> bool,
    ) -> Option<usize> {
        const _: () = assert!(mem::size_of::<StarlarkHashValue>() == mem::size_of::<u32>());
        let hashes_ints =
            unsafe { &*(self.buckets.bbb() as *const [StarlarkHashValue] as *const [u32]) };
        let mut i = 0;
        while i < hashes_ints.len() {
            i += find_hash_in_array(&hashes_ints[i..], hash.get())?;
            let k = unsafe { &self.buckets.aaa().get_unchecked(i).0 };
            if likely(eq(k)) {
                return Some(i);
            }
            i += 1;
        }
        debug_assert!(i == hashes_ints.len());
        None
    }

    #[inline]
    pub(crate) fn get_index_of_hashed<Q>(&self, key: Hashed<&Q>) -> Option<usize>
    where
        Q: ?Sized + Equivalent<K>,
    {
        self.get_index_of_hashed_raw(key.hash(), |k| key.key().equivalent(k))
    }

    #[inline]
    pub(crate) fn get_index(&self, index: usize) -> Option<(&K, &V)> {
        let ((k, v), _hash) = self.buckets.get(index)?;
        Some((k, v))
    }

    #[inline]
    pub(crate) unsafe fn get_unchecked(&self, index: usize) -> (Hashed<&K>, &V) {
        debug_assert!(index < self.buckets.len());
        let ((key, value), hash) = self.buckets.get_unchecked(index);
        (Hashed::new_unchecked(*hash, key), value)
    }

    #[inline]
    pub(crate) unsafe fn get_unchecked_mut(&mut self, index: usize) -> (Hashed<&K>, &mut V) {
        debug_assert!(index < self.buckets.len());
        let ((key, value), hash) = self.buckets.get_unchecked_mut(index);
        (Hashed::new_unchecked(*hash, key), value)
    }

    #[inline]
    pub(crate) fn insert_hashed_unique_unchecked(&mut self, key: Hashed<K>, value: V) {
        let hash = key.hash();
        self.buckets.push((key.into_key(), value), hash);
    }

    pub(crate) fn remove_hashed_entry<Q>(&mut self, key: Hashed<&Q>) -> Option<(K, V)>
    where
        Q: ?Sized + Equivalent<K>,
    {
        if let Some(index) = self.get_index_of_hashed(key) {
            let (k, v) = self.remove(index);
            Some((k.into_key(), v))
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn remove(&mut self, index: usize) -> (Hashed<K>, V) {
        let ((key, value), hash) = self.buckets.remove(index);
        (Hashed::new_unchecked(hash, key), value)
    }

    #[inline]
    pub(crate) fn pop(&mut self) -> Option<(Hashed<K>, V)> {
        let ((key, value), hash) = self.buckets.pop()?;
        Some((Hashed::new_unchecked(hash, key), value))
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.buckets.len()
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.buckets.clear();
    }

    #[inline]
    pub(crate) fn values(&self) -> Values<K, V> {
        Values { iter: self.iter() }
    }

    #[inline]
    pub(crate) fn values_mut(&mut self) -> ValuesMut<K, V> {
        ValuesMut {
            iter: self.iter_mut(),
        }
    }

    #[inline]
    pub(crate) fn keys(&self) -> Keys<K, V> {
        Keys { iter: self.iter() }
    }

    #[inline]
    pub(crate) fn into_iter(self) -> IntoIter<K, V> {
        IntoIter {
            iter: self.into_iter_hashed(),
        }
    }

    #[inline]
    pub(crate) fn iter(&self) -> Iter<K, V> {
        Iter {
            iter: self.buckets.aaa().iter(),
        }
    }

    #[inline]
    pub(crate) fn iter_hashed(&self) -> IterHashed<K, V> {
        IterHashed {
            // Values go first since they terminate first and we can short-circuit
            iter: self.buckets.iter(),
        }
    }

    #[inline]
    pub(crate) fn into_iter_hashed(self) -> IntoIterHashed<K, V> {
        // See the comments on VMIntoIterHash for why this one looks different
        IntoIterHashed {
            iter: self.buckets.into_iter(),
        }
    }

    #[inline]
    pub(crate) fn iter_mut(&mut self) -> IterMut<K, V> {
        IterMut {
            iter: self.buckets.aaa_mut().iter_mut(),
        }
    }

    #[inline]
    pub(crate) fn iter_mut_unchecked(&mut self) -> IterMutUnchecked<K, V> {
        IterMutUnchecked {
            iter: self.buckets.aaa_mut().iter_mut(),
        }
    }

    pub(crate) fn sort_keys(&mut self)
    where
        K: Ord,
    {
        self.buckets.sort_by(|(a, _ah), (b, _bh)| a.0.cmp(&b.0));
    }

    pub(crate) fn is_sorted_by_key(&self) -> bool
    where
        K: Ord,
    {
        self.buckets.aaa().windows(2).all(|w| w[0].0 <= w[1].0)
    }

    /// Equal if entries are equal in the iterator order.
    pub(crate) fn eq_ordered(&self, other: &Self) -> bool
    where
        K: PartialEq,
        V: PartialEq,
    {
        // We compare hashes before comparing keys and values because it is faster
        // (fewer branches, and no comparison of the rest it at lest one hash is different).
        self.buckets.bbb() == other.buckets.bbb() && self.buckets.aaa() == other.buckets.aaa()
    }

    /// Hash entries in the iterator order.
    ///
    /// Note, keys are not hashed, but previously computed hashes are hashed instead.
    pub(crate) fn hash_ordered<H: Hasher>(&self, state: &mut H)
    where
        K: Hash,
        V: Hash,
    {
        for e in self.iter_hashed() {
            e.hash(state);
        }
    }

    pub(crate) fn reverse(&mut self) {
        self.buckets.aaa_mut().reverse();
        self.buckets.bbb_mut().reverse();
    }

    pub(crate) fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        self.buckets.retain(|(k, v), _| f(k, v));
    }
}
