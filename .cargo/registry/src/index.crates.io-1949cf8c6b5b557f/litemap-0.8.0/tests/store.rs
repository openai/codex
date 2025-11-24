// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use litemap::store::*;
use litemap::testing::check_store_full;
use std::cmp::Ordering;

/// A Vec wrapper that leverages the default function impls from `Store`
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct VecWithDefaults<T>(Vec<T>);

type MapF<K, V> = fn(&(K, V)) -> (&K, &V);

#[inline]
fn map_f<K, V>(input: &(K, V)) -> (&K, &V) {
    (&input.0, &input.1)
}

type MapFMut<K, V> = fn(&mut (K, V)) -> (&K, &mut V);

#[inline]
fn map_f_mut<K, V>(input: &mut (K, V)) -> (&K, &mut V) {
    (&input.0, &mut input.1)
}

impl<K, V> StoreConstEmpty<K, V> for VecWithDefaults<(K, V)> {
    const EMPTY: VecWithDefaults<(K, V)> = VecWithDefaults(Vec::new());
}

impl<K, V> Store<K, V> for VecWithDefaults<(K, V)> {
    #[inline]
    fn lm_len(&self) -> usize {
        self.0.as_slice().len()
    }

    // leave lm_is_empty as default

    #[inline]
    fn lm_get(&self, index: usize) -> Option<(&K, &V)> {
        self.0.as_slice().get(index).map(map_f)
    }

    // leave lm_last as default

    #[inline]
    fn lm_binary_search_by<F>(&self, mut cmp: F) -> Result<usize, usize>
    where
        F: FnMut(&K) -> Ordering,
    {
        self.0.as_slice().binary_search_by(|(k, _)| cmp(k))
    }
}

impl<K: Ord, V> StoreFromIterable<K, V> for VecWithDefaults<(K, V)> {
    fn lm_sort_from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let v: Vec<_> = Vec::lm_sort_from_iter(iter);
        Self(v)
    }
}

impl<K, V> StoreMut<K, V> for VecWithDefaults<(K, V)> {
    #[inline]
    fn lm_with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    #[inline]
    fn lm_reserve(&mut self, additional: usize) {
        self.0.reserve(additional)
    }

    #[inline]
    fn lm_get_mut(&mut self, index: usize) -> Option<(&K, &mut V)> {
        self.0.as_mut_slice().get_mut(index).map(map_f_mut)
    }

    #[inline]
    fn lm_push(&mut self, key: K, value: V) {
        self.0.push((key, value))
    }

    #[inline]
    fn lm_insert(&mut self, index: usize, key: K, value: V) {
        self.0.insert(index, (key, value))
    }

    #[inline]
    fn lm_remove(&mut self, index: usize) -> (K, V) {
        self.0.remove(index)
    }
    #[inline]
    fn lm_clear(&mut self) {
        self.0.clear()
    }

    // leave lm_retain as default
}

impl<'a, K: 'a, V: 'a> StoreIterable<'a, K, V> for VecWithDefaults<(K, V)> {
    type KeyValueIter = core::iter::Map<core::slice::Iter<'a, (K, V)>, MapF<K, V>>;

    #[inline]
    fn lm_iter(&'a self) -> Self::KeyValueIter {
        self.0.as_slice().iter().map(map_f)
    }
}

impl<'a, K: 'a, V: 'a> StoreIterableMut<'a, K, V> for VecWithDefaults<(K, V)> {
    type KeyValueIterMut = core::iter::Map<core::slice::IterMut<'a, (K, V)>, MapFMut<K, V>>;

    #[inline]
    fn lm_iter_mut(&'a mut self) -> Self::KeyValueIterMut {
        self.0.as_mut_slice().iter_mut().map(map_f_mut)
    }
}

impl<K, V> StoreIntoIterator<K, V> for VecWithDefaults<(K, V)> {
    type KeyValueIntoIter = std::vec::IntoIter<(K, V)>;

    #[inline]
    fn lm_into_iter(self) -> Self::KeyValueIntoIter {
        IntoIterator::into_iter(self.0)
    }

    // leave lm_extend_end as default

    // leave lm_extend_start as default
}

impl<A> std::iter::FromIterator<A> for VecWithDefaults<A> {
    fn from_iter<I: IntoIterator<Item = A>>(iter: I) -> Self {
        Self(Vec::from_iter(iter))
    }
}

impl<K, V> StoreFromIterator<K, V> for VecWithDefaults<(K, V)> {}

impl<K: Ord, V> StoreBulkMut<K, V> for VecWithDefaults<(K, V)> {
    fn lm_retain<F>(&mut self, predicate: F)
    where
        F: FnMut(&K, &V) -> bool,
    {
        self.0.lm_retain(predicate)
    }

    fn lm_extend<I>(&mut self, other: I)
    where
        I: IntoIterator<Item = (K, V)>,
    {
        self.0.lm_extend(other)
    }
}

#[test]
fn test_default_impl() {
    check_store_full::<VecWithDefaults<(u32, u64)>>();
}
