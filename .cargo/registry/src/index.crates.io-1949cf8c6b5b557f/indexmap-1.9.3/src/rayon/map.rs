//! Parallel iterator types for `IndexMap` with [rayon](https://docs.rs/rayon/1.0/rayon).
//!
//! You will rarely need to interact with this module directly unless you need to name one of the
//! iterator types.
//!
//! Requires crate feature `"rayon"`

use super::collect;
use rayon::iter::plumbing::{Consumer, ProducerCallback, UnindexedConsumer};
use rayon::prelude::*;

use crate::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::ops::RangeBounds;

use crate::Bucket;
use crate::Entries;
use crate::IndexMap;

/// Requires crate feature `"rayon"`.
impl<K, V, S> IntoParallelIterator for IndexMap<K, V, S>
where
    K: Send,
    V: Send,
{
    type Item = (K, V);
    type Iter = IntoParIter<K, V>;

    fn into_par_iter(self) -> Self::Iter {
        IntoParIter {
            entries: self.into_entries(),
        }
    }
}

/// A parallel owning iterator over the entries of a `IndexMap`.
///
/// This `struct` is created by the [`into_par_iter`] method on [`IndexMap`]
/// (provided by rayon's `IntoParallelIterator` trait). See its documentation for more.
///
/// [`into_par_iter`]: ../struct.IndexMap.html#method.into_par_iter
/// [`IndexMap`]: ../struct.IndexMap.html
pub struct IntoParIter<K, V> {
    entries: Vec<Bucket<K, V>>,
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for IntoParIter<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.entries.iter().map(Bucket::refs);
        f.debug_list().entries(iter).finish()
    }
}

impl<K: Send, V: Send> ParallelIterator for IntoParIter<K, V> {
    type Item = (K, V);

    parallel_iterator_methods!(Bucket::key_value);
}

impl<K: Send, V: Send> IndexedParallelIterator for IntoParIter<K, V> {
    indexed_parallel_iterator_methods!(Bucket::key_value);
}

/// Requires crate feature `"rayon"`.
impl<'a, K, V, S> IntoParallelIterator for &'a IndexMap<K, V, S>
where
    K: Sync,
    V: Sync,
{
    type Item = (&'a K, &'a V);
    type Iter = ParIter<'a, K, V>;

    fn into_par_iter(self) -> Self::Iter {
        ParIter {
            entries: self.as_entries(),
        }
    }
}

/// A parallel iterator over the entries of a `IndexMap`.
///
/// This `struct` is created by the [`par_iter`] method on [`IndexMap`]
/// (provided by rayon's `IntoParallelRefIterator` trait). See its documentation for more.
///
/// [`par_iter`]: ../struct.IndexMap.html#method.par_iter
/// [`IndexMap`]: ../struct.IndexMap.html
pub struct ParIter<'a, K, V> {
    entries: &'a [Bucket<K, V>],
}

impl<K, V> Clone for ParIter<'_, K, V> {
    fn clone(&self) -> Self {
        ParIter { ..*self }
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for ParIter<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.entries.iter().map(Bucket::refs);
        f.debug_list().entries(iter).finish()
    }
}

impl<'a, K: Sync, V: Sync> ParallelIterator for ParIter<'a, K, V> {
    type Item = (&'a K, &'a V);

    parallel_iterator_methods!(Bucket::refs);
}

impl<K: Sync, V: Sync> IndexedParallelIterator for ParIter<'_, K, V> {
    indexed_parallel_iterator_methods!(Bucket::refs);
}

/// Requires crate feature `"rayon"`.
impl<'a, K, V, S> IntoParallelIterator for &'a mut IndexMap<K, V, S>
where
    K: Sync + Send,
    V: Send,
{
    type Item = (&'a K, &'a mut V);
    type Iter = ParIterMut<'a, K, V>;

    fn into_par_iter(self) -> Self::Iter {
        ParIterMut {
            entries: self.as_entries_mut(),
        }
    }
}

/// A parallel mutable iterator over the entries of a `IndexMap`.
///
/// This `struct` is created by the [`par_iter_mut`] method on [`IndexMap`]
/// (provided by rayon's `IntoParallelRefMutIterator` trait). See its documentation for more.
///
/// [`par_iter_mut`]: ../struct.IndexMap.html#method.par_iter_mut
/// [`IndexMap`]: ../struct.IndexMap.html
pub struct ParIterMut<'a, K, V> {
    entries: &'a mut [Bucket<K, V>],
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for ParIterMut<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.entries.iter().map(Bucket::refs);
        f.debug_list().entries(iter).finish()
    }
}

impl<'a, K: Sync + Send, V: Send> ParallelIterator for ParIterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    parallel_iterator_methods!(Bucket::ref_mut);
}

impl<K: Sync + Send, V: Send> IndexedParallelIterator for ParIterMut<'_, K, V> {
    indexed_parallel_iterator_methods!(Bucket::ref_mut);
}

/// Requires crate feature `"rayon"`.
impl<'a, K, V, S> ParallelDrainRange<usize> for &'a mut IndexMap<K, V, S>
where
    K: Send,
    V: Send,
{
    type Item = (K, V);
    type Iter = ParDrain<'a, K, V>;

    fn par_drain<R: RangeBounds<usize>>(self, range: R) -> Self::Iter {
        ParDrain {
            entries: self.core.par_drain(range),
        }
    }
}

/// A parallel draining iterator over the entries of a `IndexMap`.
///
/// This `struct` is created by the [`par_drain`] method on [`IndexMap`]
/// (provided by rayon's `ParallelDrainRange` trait). See its documentation for more.
///
/// [`par_drain`]: ../struct.IndexMap.html#method.par_drain
/// [`IndexMap`]: ../struct.IndexMap.html
pub struct ParDrain<'a, K: Send, V: Send> {
    entries: rayon::vec::Drain<'a, Bucket<K, V>>,
}

impl<K: Send, V: Send> ParallelIterator for ParDrain<'_, K, V> {
    type Item = (K, V);

    parallel_iterator_methods!(Bucket::key_value);
}

impl<K: Send, V: Send> IndexedParallelIterator for ParDrain<'_, K, V> {
    indexed_parallel_iterator_methods!(Bucket::key_value);
}

/// Parallel iterator methods and other parallel methods.
///
/// The following methods **require crate feature `"rayon"`**.
///
/// See also the `IntoParallelIterator` implementations.
impl<K, V, S> IndexMap<K, V, S>
where
    K: Sync,
    V: Sync,
{
    /// Return a parallel iterator over the keys of the map.
    ///
    /// While parallel iterators can process items in any order, their relative order
    /// in the map is still preserved for operations like `reduce` and `collect`.
    pub fn par_keys(&self) -> ParKeys<'_, K, V> {
        ParKeys {
            entries: self.as_entries(),
        }
    }

    /// Return a parallel iterator over the values of the map.
    ///
    /// While parallel iterators can process items in any order, their relative order
    /// in the map is still preserved for operations like `reduce` and `collect`.
    pub fn par_values(&self) -> ParValues<'_, K, V> {
        ParValues {
            entries: self.as_entries(),
        }
    }
}

impl<K, V, S> IndexMap<K, V, S>
where
    K: Hash + Eq + Sync,
    V: Sync,
    S: BuildHasher,
{
    /// Returns `true` if `self` contains all of the same key-value pairs as `other`,
    /// regardless of each map's indexed order, determined in parallel.
    pub fn par_eq<V2, S2>(&self, other: &IndexMap<K, V2, S2>) -> bool
    where
        V: PartialEq<V2>,
        V2: Sync,
        S2: BuildHasher + Sync,
    {
        self.len() == other.len()
            && self
                .par_iter()
                .all(move |(key, value)| other.get(key).map_or(false, |v| *value == *v))
    }
}

/// A parallel iterator over the keys of a `IndexMap`.
///
/// This `struct` is created by the [`par_keys`] method on [`IndexMap`]. See its
/// documentation for more.
///
/// [`par_keys`]: ../struct.IndexMap.html#method.par_keys
/// [`IndexMap`]: ../struct.IndexMap.html
pub struct ParKeys<'a, K, V> {
    entries: &'a [Bucket<K, V>],
}

impl<K, V> Clone for ParKeys<'_, K, V> {
    fn clone(&self) -> Self {
        ParKeys { ..*self }
    }
}

impl<K: fmt::Debug, V> fmt::Debug for ParKeys<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.entries.iter().map(Bucket::key_ref);
        f.debug_list().entries(iter).finish()
    }
}

impl<'a, K: Sync, V: Sync> ParallelIterator for ParKeys<'a, K, V> {
    type Item = &'a K;

    parallel_iterator_methods!(Bucket::key_ref);
}

impl<K: Sync, V: Sync> IndexedParallelIterator for ParKeys<'_, K, V> {
    indexed_parallel_iterator_methods!(Bucket::key_ref);
}

/// A parallel iterator over the values of a `IndexMap`.
///
/// This `struct` is created by the [`par_values`] method on [`IndexMap`]. See its
/// documentation for more.
///
/// [`par_values`]: ../struct.IndexMap.html#method.par_values
/// [`IndexMap`]: ../struct.IndexMap.html
pub struct ParValues<'a, K, V> {
    entries: &'a [Bucket<K, V>],
}

impl<K, V> Clone for ParValues<'_, K, V> {
    fn clone(&self) -> Self {
        ParValues { ..*self }
    }
}

impl<K, V: fmt::Debug> fmt::Debug for ParValues<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.entries.iter().map(Bucket::value_ref);
        f.debug_list().entries(iter).finish()
    }
}

impl<'a, K: Sync, V: Sync> ParallelIterator for ParValues<'a, K, V> {
    type Item = &'a V;

    parallel_iterator_methods!(Bucket::value_ref);
}

impl<K: Sync, V: Sync> IndexedParallelIterator for ParValues<'_, K, V> {
    indexed_parallel_iterator_methods!(Bucket::value_ref);
}

/// Requires crate feature `"rayon"`.
impl<K, V, S> IndexMap<K, V, S>
where
    K: Send,
    V: Send,
{
    /// Return a parallel iterator over mutable references to the values of the map
    ///
    /// While parallel iterators can process items in any order, their relative order
    /// in the map is still preserved for operations like `reduce` and `collect`.
    pub fn par_values_mut(&mut self) -> ParValuesMut<'_, K, V> {
        ParValuesMut {
            entries: self.as_entries_mut(),
        }
    }
}

impl<K, V, S> IndexMap<K, V, S>
where
    K: Hash + Eq + Send,
    V: Send,
    S: BuildHasher,
{
    /// Sort the map’s key-value pairs in parallel, by the default ordering of the keys.
    pub fn par_sort_keys(&mut self)
    where
        K: Ord,
    {
        self.with_entries(|entries| {
            entries.par_sort_by(|a, b| K::cmp(&a.key, &b.key));
        });
    }

    /// Sort the map’s key-value pairs in place and in parallel, using the comparison
    /// function `cmp`.
    ///
    /// The comparison function receives two key and value pairs to compare (you
    /// can sort by keys or values or their combination as needed).
    pub fn par_sort_by<F>(&mut self, cmp: F)
    where
        F: Fn(&K, &V, &K, &V) -> Ordering + Sync,
    {
        self.with_entries(|entries| {
            entries.par_sort_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value));
        });
    }

    /// Sort the key-value pairs of the map in parallel and return a by-value parallel
    /// iterator of the key-value pairs with the result.
    pub fn par_sorted_by<F>(self, cmp: F) -> IntoParIter<K, V>
    where
        F: Fn(&K, &V, &K, &V) -> Ordering + Sync,
    {
        let mut entries = self.into_entries();
        entries.par_sort_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value));
        IntoParIter { entries }
    }

    /// Sort the map's key-value pairs in parallel, by the default ordering of the keys.
    pub fn par_sort_unstable_keys(&mut self)
    where
        K: Ord,
    {
        self.with_entries(|entries| {
            entries.par_sort_unstable_by(|a, b| K::cmp(&a.key, &b.key));
        });
    }

    /// Sort the map's key-value pairs in place and in parallel, using the comparison
    /// function `cmp`.
    ///
    /// The comparison function receives two key and value pairs to compare (you
    /// can sort by keys or values or their combination as needed).
    pub fn par_sort_unstable_by<F>(&mut self, cmp: F)
    where
        F: Fn(&K, &V, &K, &V) -> Ordering + Sync,
    {
        self.with_entries(|entries| {
            entries.par_sort_unstable_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value));
        });
    }

    /// Sort the key-value pairs of the map in parallel and return a by-value parallel
    /// iterator of the key-value pairs with the result.
    pub fn par_sorted_unstable_by<F>(self, cmp: F) -> IntoParIter<K, V>
    where
        F: Fn(&K, &V, &K, &V) -> Ordering + Sync,
    {
        let mut entries = self.into_entries();
        entries.par_sort_unstable_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value));
        IntoParIter { entries }
    }
}

/// A parallel mutable iterator over the values of a `IndexMap`.
///
/// This `struct` is created by the [`par_values_mut`] method on [`IndexMap`]. See its
/// documentation for more.
///
/// [`par_values_mut`]: ../struct.IndexMap.html#method.par_values_mut
/// [`IndexMap`]: ../struct.IndexMap.html
pub struct ParValuesMut<'a, K, V> {
    entries: &'a mut [Bucket<K, V>],
}

impl<K, V: fmt::Debug> fmt::Debug for ParValuesMut<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.entries.iter().map(Bucket::value_ref);
        f.debug_list().entries(iter).finish()
    }
}

impl<'a, K: Send, V: Send> ParallelIterator for ParValuesMut<'a, K, V> {
    type Item = &'a mut V;

    parallel_iterator_methods!(Bucket::value_mut);
}

impl<K: Send, V: Send> IndexedParallelIterator for ParValuesMut<'_, K, V> {
    indexed_parallel_iterator_methods!(Bucket::value_mut);
}

/// Requires crate feature `"rayon"`.
impl<K, V, S> FromParallelIterator<(K, V)> for IndexMap<K, V, S>
where
    K: Eq + Hash + Send,
    V: Send,
    S: BuildHasher + Default + Send,
{
    fn from_par_iter<I>(iter: I) -> Self
    where
        I: IntoParallelIterator<Item = (K, V)>,
    {
        let list = collect(iter);
        let len = list.iter().map(Vec::len).sum();
        let mut map = Self::with_capacity_and_hasher(len, S::default());
        for vec in list {
            map.extend(vec);
        }
        map
    }
}

/// Requires crate feature `"rayon"`.
impl<K, V, S> ParallelExtend<(K, V)> for IndexMap<K, V, S>
where
    K: Eq + Hash + Send,
    V: Send,
    S: BuildHasher + Send,
{
    fn par_extend<I>(&mut self, iter: I)
    where
        I: IntoParallelIterator<Item = (K, V)>,
    {
        for vec in collect(iter) {
            self.extend(vec);
        }
    }
}

/// Requires crate feature `"rayon"`.
impl<'a, K: 'a, V: 'a, S> ParallelExtend<(&'a K, &'a V)> for IndexMap<K, V, S>
where
    K: Copy + Eq + Hash + Send + Sync,
    V: Copy + Send + Sync,
    S: BuildHasher + Send,
{
    fn par_extend<I>(&mut self, iter: I)
    where
        I: IntoParallelIterator<Item = (&'a K, &'a V)>,
    {
        for vec in collect(iter) {
            self.extend(vec);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::String;

    #[test]
    fn insert_order() {
        let insert = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
        let mut map = IndexMap::new();

        for &elt in &insert {
            map.insert(elt, ());
        }

        assert_eq!(map.par_keys().count(), map.len());
        assert_eq!(map.par_keys().count(), insert.len());
        insert.par_iter().zip(map.par_keys()).for_each(|(a, b)| {
            assert_eq!(a, b);
        });
        (0..insert.len())
            .into_par_iter()
            .zip(map.par_keys())
            .for_each(|(i, k)| {
                assert_eq!(map.get_index(i).unwrap().0, k);
            });
    }

    #[test]
    fn partial_eq_and_eq() {
        let mut map_a = IndexMap::new();
        map_a.insert(1, "1");
        map_a.insert(2, "2");
        let mut map_b = map_a.clone();
        assert!(map_a.par_eq(&map_b));
        map_b.swap_remove(&1);
        assert!(!map_a.par_eq(&map_b));
        map_b.insert(3, "3");
        assert!(!map_a.par_eq(&map_b));

        let map_c: IndexMap<_, String> =
            map_b.into_par_iter().map(|(k, v)| (k, v.into())).collect();
        assert!(!map_a.par_eq(&map_c));
        assert!(!map_c.par_eq(&map_a));
    }

    #[test]
    fn extend() {
        let mut map = IndexMap::new();
        map.par_extend(vec![(&1, &2), (&3, &4)]);
        map.par_extend(vec![(5, 6)]);
        assert_eq!(
            map.into_par_iter().collect::<Vec<_>>(),
            vec![(1, 2), (3, 4), (5, 6)]
        );
    }

    #[test]
    fn keys() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: IndexMap<_, _> = vec.into_par_iter().collect();
        let keys: Vec<_> = map.par_keys().copied().collect();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&1));
        assert!(keys.contains(&2));
        assert!(keys.contains(&3));
    }

    #[test]
    fn values() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: IndexMap<_, _> = vec.into_par_iter().collect();
        let values: Vec<_> = map.par_values().copied().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&'a'));
        assert!(values.contains(&'b'));
        assert!(values.contains(&'c'));
    }

    #[test]
    fn values_mut() {
        let vec = vec![(1, 1), (2, 2), (3, 3)];
        let mut map: IndexMap<_, _> = vec.into_par_iter().collect();
        map.par_values_mut().for_each(|value| *value *= 2);
        let values: Vec<_> = map.par_values().copied().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&2));
        assert!(values.contains(&4));
        assert!(values.contains(&6));
    }
}
