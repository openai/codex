//! Parallel iterator types for [`IndexSet`] with [rayon][::rayon].
//!
//! You will rarely need to interact with this module directly unless you need to name one of the
//! iterator types.

use super::collect;
use rayon::iter::plumbing::{Consumer, ProducerCallback, UnindexedConsumer};
use rayon::prelude::*;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::ops::RangeBounds;

use crate::set::Slice;
use crate::IndexSet;

type Bucket<T> = crate::Bucket<T, ()>;

impl<T, S> IntoParallelIterator for IndexSet<T, S>
where
    T: Send,
{
    type Item = T;
    type Iter = IntoParIter<T>;

    fn into_par_iter(self) -> Self::Iter {
        IntoParIter {
            entries: self.into_entries(),
        }
    }
}

impl<T> IntoParallelIterator for Box<Slice<T>>
where
    T: Send,
{
    type Item = T;
    type Iter = IntoParIter<T>;

    fn into_par_iter(self) -> Self::Iter {
        IntoParIter {
            entries: self.into_entries(),
        }
    }
}

/// A parallel owning iterator over the items of an [`IndexSet`].
///
/// This `struct` is created by the [`IndexSet::into_par_iter`] method
/// (provided by rayon's [`IntoParallelIterator`] trait). See its documentation for more.
pub struct IntoParIter<T> {
    entries: Vec<Bucket<T>>,
}

impl<T: fmt::Debug> fmt::Debug for IntoParIter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.entries.iter().map(Bucket::key_ref);
        f.debug_list().entries(iter).finish()
    }
}

impl<T: Send> ParallelIterator for IntoParIter<T> {
    type Item = T;

    parallel_iterator_methods!(Bucket::key);
}

impl<T: Send> IndexedParallelIterator for IntoParIter<T> {
    indexed_parallel_iterator_methods!(Bucket::key);
}

impl<'a, T, S> IntoParallelIterator for &'a IndexSet<T, S>
where
    T: Sync,
{
    type Item = &'a T;
    type Iter = ParIter<'a, T>;

    fn into_par_iter(self) -> Self::Iter {
        ParIter {
            entries: self.as_entries(),
        }
    }
}

impl<'a, T> IntoParallelIterator for &'a Slice<T>
where
    T: Sync,
{
    type Item = &'a T;
    type Iter = ParIter<'a, T>;

    fn into_par_iter(self) -> Self::Iter {
        ParIter {
            entries: &self.entries,
        }
    }
}

/// A parallel iterator over the items of an [`IndexSet`].
///
/// This `struct` is created by the [`IndexSet::par_iter`] method
/// (provided by rayon's [`IntoParallelRefIterator`] trait). See its documentation for more.
///
/// [`IndexSet::par_iter`]: ../struct.IndexSet.html#method.par_iter
pub struct ParIter<'a, T> {
    entries: &'a [Bucket<T>],
}

impl<T> Clone for ParIter<'_, T> {
    fn clone(&self) -> Self {
        ParIter { ..*self }
    }
}

impl<T: fmt::Debug> fmt::Debug for ParIter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.entries.iter().map(Bucket::key_ref);
        f.debug_list().entries(iter).finish()
    }
}

impl<'a, T: Sync> ParallelIterator for ParIter<'a, T> {
    type Item = &'a T;

    parallel_iterator_methods!(Bucket::key_ref);
}

impl<T: Sync> IndexedParallelIterator for ParIter<'_, T> {
    indexed_parallel_iterator_methods!(Bucket::key_ref);
}

impl<'a, T, S> ParallelDrainRange<usize> for &'a mut IndexSet<T, S>
where
    T: Send,
{
    type Item = T;
    type Iter = ParDrain<'a, T>;

    fn par_drain<R: RangeBounds<usize>>(self, range: R) -> Self::Iter {
        ParDrain {
            entries: self.map.core.par_drain(range),
        }
    }
}

/// A parallel draining iterator over the items of an [`IndexSet`].
///
/// This `struct` is created by the [`IndexSet::par_drain`] method
/// (provided by rayon's [`ParallelDrainRange`] trait). See its documentation for more.
///
/// [`IndexSet::par_drain`]: ../struct.IndexSet.html#method.par_drain
pub struct ParDrain<'a, T: Send> {
    entries: rayon::vec::Drain<'a, Bucket<T>>,
}

impl<T: Send> ParallelIterator for ParDrain<'_, T> {
    type Item = T;

    parallel_iterator_methods!(Bucket::key);
}

impl<T: Send> IndexedParallelIterator for ParDrain<'_, T> {
    indexed_parallel_iterator_methods!(Bucket::key);
}

/// Parallel iterator methods and other parallel methods.
///
/// The following methods **require crate feature `"rayon"`**.
///
/// See also the `IntoParallelIterator` implementations.
impl<T, S> IndexSet<T, S>
where
    T: Hash + Eq + Sync,
    S: BuildHasher + Sync,
{
    /// Return a parallel iterator over the values that are in `self` but not `other`.
    ///
    /// While parallel iterators can process items in any order, their relative order
    /// in the `self` set is still preserved for operations like `reduce` and `collect`.
    pub fn par_difference<'a, S2>(
        &'a self,
        other: &'a IndexSet<T, S2>,
    ) -> ParDifference<'a, T, S, S2>
    where
        S2: BuildHasher + Sync,
    {
        ParDifference {
            set1: self,
            set2: other,
        }
    }

    /// Return a parallel iterator over the values that are in `self` or `other`,
    /// but not in both.
    ///
    /// While parallel iterators can process items in any order, their relative order
    /// in the sets is still preserved for operations like `reduce` and `collect`.
    /// Values from `self` are produced in their original order, followed by
    /// values from `other` in their original order.
    pub fn par_symmetric_difference<'a, S2>(
        &'a self,
        other: &'a IndexSet<T, S2>,
    ) -> ParSymmetricDifference<'a, T, S, S2>
    where
        S2: BuildHasher + Sync,
    {
        ParSymmetricDifference {
            set1: self,
            set2: other,
        }
    }

    /// Return a parallel iterator over the values that are in both `self` and `other`.
    ///
    /// While parallel iterators can process items in any order, their relative order
    /// in the `self` set is still preserved for operations like `reduce` and `collect`.
    pub fn par_intersection<'a, S2>(
        &'a self,
        other: &'a IndexSet<T, S2>,
    ) -> ParIntersection<'a, T, S, S2>
    where
        S2: BuildHasher + Sync,
    {
        ParIntersection {
            set1: self,
            set2: other,
        }
    }

    /// Return a parallel iterator over all values that are in `self` or `other`.
    ///
    /// While parallel iterators can process items in any order, their relative order
    /// in the sets is still preserved for operations like `reduce` and `collect`.
    /// Values from `self` are produced in their original order, followed by
    /// values that are unique to `other` in their original order.
    pub fn par_union<'a, S2>(&'a self, other: &'a IndexSet<T, S2>) -> ParUnion<'a, T, S, S2>
    where
        S2: BuildHasher + Sync,
    {
        ParUnion {
            set1: self,
            set2: other,
        }
    }

    /// Returns `true` if `self` contains all of the same values as `other`,
    /// regardless of each set's indexed order, determined in parallel.
    pub fn par_eq<S2>(&self, other: &IndexSet<T, S2>) -> bool
    where
        S2: BuildHasher + Sync,
    {
        self.len() == other.len() && self.par_is_subset(other)
    }

    /// Returns `true` if `self` has no elements in common with `other`,
    /// determined in parallel.
    pub fn par_is_disjoint<S2>(&self, other: &IndexSet<T, S2>) -> bool
    where
        S2: BuildHasher + Sync,
    {
        if self.len() <= other.len() {
            self.par_iter().all(move |value| !other.contains(value))
        } else {
            other.par_iter().all(move |value| !self.contains(value))
        }
    }

    /// Returns `true` if all elements of `other` are contained in `self`,
    /// determined in parallel.
    pub fn par_is_superset<S2>(&self, other: &IndexSet<T, S2>) -> bool
    where
        S2: BuildHasher + Sync,
    {
        other.par_is_subset(self)
    }

    /// Returns `true` if all elements of `self` are contained in `other`,
    /// determined in parallel.
    pub fn par_is_subset<S2>(&self, other: &IndexSet<T, S2>) -> bool
    where
        S2: BuildHasher + Sync,
    {
        self.len() <= other.len() && self.par_iter().all(move |value| other.contains(value))
    }
}

/// A parallel iterator producing elements in the difference of [`IndexSet`]s.
///
/// This `struct` is created by the [`IndexSet::par_difference`] method.
/// See its documentation for more.
pub struct ParDifference<'a, T, S1, S2> {
    set1: &'a IndexSet<T, S1>,
    set2: &'a IndexSet<T, S2>,
}

impl<T, S1, S2> Clone for ParDifference<'_, T, S1, S2> {
    fn clone(&self) -> Self {
        ParDifference { ..*self }
    }
}

impl<T, S1, S2> fmt::Debug for ParDifference<'_, T, S1, S2>
where
    T: fmt::Debug + Eq + Hash,
    S1: BuildHasher,
    S2: BuildHasher,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.set1.difference(self.set2))
            .finish()
    }
}

impl<'a, T, S1, S2> ParallelIterator for ParDifference<'a, T, S1, S2>
where
    T: Hash + Eq + Sync,
    S1: BuildHasher + Sync,
    S2: BuildHasher + Sync,
{
    type Item = &'a T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let Self { set1, set2 } = self;

        set1.par_iter()
            .filter(move |&item| !set2.contains(item))
            .drive_unindexed(consumer)
    }
}

/// A parallel iterator producing elements in the intersection of [`IndexSet`]s.
///
/// This `struct` is created by the [`IndexSet::par_intersection`] method.
/// See its documentation for more.
pub struct ParIntersection<'a, T, S1, S2> {
    set1: &'a IndexSet<T, S1>,
    set2: &'a IndexSet<T, S2>,
}

impl<T, S1, S2> Clone for ParIntersection<'_, T, S1, S2> {
    fn clone(&self) -> Self {
        ParIntersection { ..*self }
    }
}

impl<T, S1, S2> fmt::Debug for ParIntersection<'_, T, S1, S2>
where
    T: fmt::Debug + Eq + Hash,
    S1: BuildHasher,
    S2: BuildHasher,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.set1.intersection(self.set2))
            .finish()
    }
}

impl<'a, T, S1, S2> ParallelIterator for ParIntersection<'a, T, S1, S2>
where
    T: Hash + Eq + Sync,
    S1: BuildHasher + Sync,
    S2: BuildHasher + Sync,
{
    type Item = &'a T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let Self { set1, set2 } = self;

        set1.par_iter()
            .filter(move |&item| set2.contains(item))
            .drive_unindexed(consumer)
    }
}

/// A parallel iterator producing elements in the symmetric difference of [`IndexSet`]s.
///
/// This `struct` is created by the [`IndexSet::par_symmetric_difference`] method.
/// See its documentation for more.
pub struct ParSymmetricDifference<'a, T, S1, S2> {
    set1: &'a IndexSet<T, S1>,
    set2: &'a IndexSet<T, S2>,
}

impl<T, S1, S2> Clone for ParSymmetricDifference<'_, T, S1, S2> {
    fn clone(&self) -> Self {
        ParSymmetricDifference { ..*self }
    }
}

impl<T, S1, S2> fmt::Debug for ParSymmetricDifference<'_, T, S1, S2>
where
    T: fmt::Debug + Eq + Hash,
    S1: BuildHasher,
    S2: BuildHasher,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.set1.symmetric_difference(self.set2))
            .finish()
    }
}

impl<'a, T, S1, S2> ParallelIterator for ParSymmetricDifference<'a, T, S1, S2>
where
    T: Hash + Eq + Sync,
    S1: BuildHasher + Sync,
    S2: BuildHasher + Sync,
{
    type Item = &'a T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let Self { set1, set2 } = self;

        set1.par_difference(set2)
            .chain(set2.par_difference(set1))
            .drive_unindexed(consumer)
    }
}

/// A parallel iterator producing elements in the union of [`IndexSet`]s.
///
/// This `struct` is created by the [`IndexSet::par_union`] method.
/// See its documentation for more.
pub struct ParUnion<'a, T, S1, S2> {
    set1: &'a IndexSet<T, S1>,
    set2: &'a IndexSet<T, S2>,
}

impl<T, S1, S2> Clone for ParUnion<'_, T, S1, S2> {
    fn clone(&self) -> Self {
        ParUnion { ..*self }
    }
}

impl<T, S1, S2> fmt::Debug for ParUnion<'_, T, S1, S2>
where
    T: fmt::Debug + Eq + Hash,
    S1: BuildHasher,
    S2: BuildHasher,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.set1.union(self.set2)).finish()
    }
}

impl<'a, T, S1, S2> ParallelIterator for ParUnion<'a, T, S1, S2>
where
    T: Hash + Eq + Sync,
    S1: BuildHasher + Sync,
    S2: BuildHasher + Sync,
{
    type Item = &'a T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let Self { set1, set2 } = self;

        set1.par_iter()
            .chain(set2.par_difference(set1))
            .drive_unindexed(consumer)
    }
}

/// Parallel sorting methods.
///
/// The following methods **require crate feature `"rayon"`**.
impl<T, S> IndexSet<T, S>
where
    T: Send,
{
    /// Sort the set's values in parallel by their default ordering.
    pub fn par_sort(&mut self)
    where
        T: Ord,
    {
        self.with_entries(|entries| {
            entries.par_sort_by(|a, b| T::cmp(&a.key, &b.key));
        });
    }

    /// Sort the set's values in place and in parallel, using the comparison function `cmp`.
    pub fn par_sort_by<F>(&mut self, cmp: F)
    where
        F: Fn(&T, &T) -> Ordering + Sync,
    {
        self.with_entries(|entries| {
            entries.par_sort_by(move |a, b| cmp(&a.key, &b.key));
        });
    }

    /// Sort the values of the set in parallel and return a by-value parallel iterator of
    /// the values with the result.
    pub fn par_sorted_by<F>(self, cmp: F) -> IntoParIter<T>
    where
        F: Fn(&T, &T) -> Ordering + Sync,
    {
        let mut entries = self.into_entries();
        entries.par_sort_by(move |a, b| cmp(&a.key, &b.key));
        IntoParIter { entries }
    }

    /// Sort the set's values in place and in parallel, using a key extraction function.
    pub fn par_sort_by_key<K, F>(&mut self, sort_key: F)
    where
        K: Ord,
        F: Fn(&T) -> K + Sync,
    {
        self.with_entries(move |entries| {
            entries.par_sort_by_key(move |a| sort_key(&a.key));
        });
    }

    /// Sort the set's values in parallel by their default ordering.
    pub fn par_sort_unstable(&mut self)
    where
        T: Ord,
    {
        self.with_entries(|entries| {
            entries.par_sort_unstable_by(|a, b| T::cmp(&a.key, &b.key));
        });
    }

    /// Sort the set's values in place and in parallel, using the comparison function `cmp`.
    pub fn par_sort_unstable_by<F>(&mut self, cmp: F)
    where
        F: Fn(&T, &T) -> Ordering + Sync,
    {
        self.with_entries(|entries| {
            entries.par_sort_unstable_by(move |a, b| cmp(&a.key, &b.key));
        });
    }

    /// Sort the values of the set in parallel and return a by-value parallel iterator of
    /// the values with the result.
    pub fn par_sorted_unstable_by<F>(self, cmp: F) -> IntoParIter<T>
    where
        F: Fn(&T, &T) -> Ordering + Sync,
    {
        let mut entries = self.into_entries();
        entries.par_sort_unstable_by(move |a, b| cmp(&a.key, &b.key));
        IntoParIter { entries }
    }

    /// Sort the set's values in place and in parallel, using a key extraction function.
    pub fn par_sort_unstable_by_key<K, F>(&mut self, sort_key: F)
    where
        K: Ord,
        F: Fn(&T) -> K + Sync,
    {
        self.with_entries(move |entries| {
            entries.par_sort_unstable_by_key(move |a| sort_key(&a.key));
        });
    }

    /// Sort the set's values in place and in parallel, using a key extraction function.
    pub fn par_sort_by_cached_key<K, F>(&mut self, sort_key: F)
    where
        K: Ord + Send,
        F: Fn(&T) -> K + Sync,
    {
        self.with_entries(move |entries| {
            entries.par_sort_by_cached_key(move |a| sort_key(&a.key));
        });
    }
}

impl<T, S> FromParallelIterator<T> for IndexSet<T, S>
where
    T: Eq + Hash + Send,
    S: BuildHasher + Default + Send,
{
    fn from_par_iter<I>(iter: I) -> Self
    where
        I: IntoParallelIterator<Item = T>,
    {
        let list = collect(iter);
        let len = list.iter().map(Vec::len).sum();
        let mut set = Self::with_capacity_and_hasher(len, S::default());
        for vec in list {
            set.extend(vec);
        }
        set
    }
}

impl<T, S> ParallelExtend<T> for IndexSet<T, S>
where
    T: Eq + Hash + Send,
    S: BuildHasher + Send,
{
    fn par_extend<I>(&mut self, iter: I)
    where
        I: IntoParallelIterator<Item = T>,
    {
        for vec in collect(iter) {
            self.extend(vec);
        }
    }
}

impl<'a, T: 'a, S> ParallelExtend<&'a T> for IndexSet<T, S>
where
    T: Copy + Eq + Hash + Send + Sync,
    S: BuildHasher + Send,
{
    fn par_extend<I>(&mut self, iter: I)
    where
        I: IntoParallelIterator<Item = &'a T>,
    {
        for vec in collect(iter) {
            self.extend(vec);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_order() {
        let insert = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
        let mut set = IndexSet::new();

        for &elt in &insert {
            set.insert(elt);
        }

        assert_eq!(set.par_iter().count(), set.len());
        assert_eq!(set.par_iter().count(), insert.len());
        insert.par_iter().zip(&set).for_each(|(a, b)| {
            assert_eq!(a, b);
        });
        (0..insert.len())
            .into_par_iter()
            .zip(&set)
            .for_each(|(i, v)| {
                assert_eq!(set.get_index(i).unwrap(), v);
            });
    }

    #[test]
    fn partial_eq_and_eq() {
        let mut set_a = IndexSet::new();
        set_a.insert(1);
        set_a.insert(2);
        let mut set_b = set_a.clone();
        assert!(set_a.par_eq(&set_b));
        set_b.swap_remove(&1);
        assert!(!set_a.par_eq(&set_b));
        set_b.insert(3);
        assert!(!set_a.par_eq(&set_b));

        let set_c: IndexSet<_> = set_b.into_par_iter().collect();
        assert!(!set_a.par_eq(&set_c));
        assert!(!set_c.par_eq(&set_a));
    }

    #[test]
    fn extend() {
        let mut set = IndexSet::new();
        set.par_extend(vec![&1, &2, &3, &4]);
        set.par_extend(vec![5, 6]);
        assert_eq!(
            set.into_par_iter().collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5, 6]
        );
    }

    #[test]
    fn comparisons() {
        let set_a: IndexSet<_> = (0..3).collect();
        let set_b: IndexSet<_> = (3..6).collect();
        let set_c: IndexSet<_> = (0..6).collect();
        let set_d: IndexSet<_> = (3..9).collect();

        assert!(!set_a.par_is_disjoint(&set_a));
        assert!(set_a.par_is_subset(&set_a));
        assert!(set_a.par_is_superset(&set_a));

        assert!(set_a.par_is_disjoint(&set_b));
        assert!(set_b.par_is_disjoint(&set_a));
        assert!(!set_a.par_is_subset(&set_b));
        assert!(!set_b.par_is_subset(&set_a));
        assert!(!set_a.par_is_superset(&set_b));
        assert!(!set_b.par_is_superset(&set_a));

        assert!(!set_a.par_is_disjoint(&set_c));
        assert!(!set_c.par_is_disjoint(&set_a));
        assert!(set_a.par_is_subset(&set_c));
        assert!(!set_c.par_is_subset(&set_a));
        assert!(!set_a.par_is_superset(&set_c));
        assert!(set_c.par_is_superset(&set_a));

        assert!(!set_c.par_is_disjoint(&set_d));
        assert!(!set_d.par_is_disjoint(&set_c));
        assert!(!set_c.par_is_subset(&set_d));
        assert!(!set_d.par_is_subset(&set_c));
        assert!(!set_c.par_is_superset(&set_d));
        assert!(!set_d.par_is_superset(&set_c));
    }

    #[test]
    fn iter_comparisons() {
        use std::iter::empty;

        fn check<'a, I1, I2>(iter1: I1, iter2: I2)
        where
            I1: ParallelIterator<Item = &'a i32>,
            I2: Iterator<Item = i32>,
        {
            let v1: Vec<_> = iter1.copied().collect();
            let v2: Vec<_> = iter2.collect();
            assert_eq!(v1, v2);
        }

        let set_a: IndexSet<_> = (0..3).collect();
        let set_b: IndexSet<_> = (3..6).collect();
        let set_c: IndexSet<_> = (0..6).collect();
        let set_d: IndexSet<_> = (3..9).rev().collect();

        check(set_a.par_difference(&set_a), empty());
        check(set_a.par_symmetric_difference(&set_a), empty());
        check(set_a.par_intersection(&set_a), 0..3);
        check(set_a.par_union(&set_a), 0..3);

        check(set_a.par_difference(&set_b), 0..3);
        check(set_b.par_difference(&set_a), 3..6);
        check(set_a.par_symmetric_difference(&set_b), 0..6);
        check(set_b.par_symmetric_difference(&set_a), (3..6).chain(0..3));
        check(set_a.par_intersection(&set_b), empty());
        check(set_b.par_intersection(&set_a), empty());
        check(set_a.par_union(&set_b), 0..6);
        check(set_b.par_union(&set_a), (3..6).chain(0..3));

        check(set_a.par_difference(&set_c), empty());
        check(set_c.par_difference(&set_a), 3..6);
        check(set_a.par_symmetric_difference(&set_c), 3..6);
        check(set_c.par_symmetric_difference(&set_a), 3..6);
        check(set_a.par_intersection(&set_c), 0..3);
        check(set_c.par_intersection(&set_a), 0..3);
        check(set_a.par_union(&set_c), 0..6);
        check(set_c.par_union(&set_a), 0..6);

        check(set_c.par_difference(&set_d), 0..3);
        check(set_d.par_difference(&set_c), (6..9).rev());
        check(
            set_c.par_symmetric_difference(&set_d),
            (0..3).chain((6..9).rev()),
        );
        check(
            set_d.par_symmetric_difference(&set_c),
            (6..9).rev().chain(0..3),
        );
        check(set_c.par_intersection(&set_d), 3..6);
        check(set_d.par_intersection(&set_c), (3..6).rev());
        check(set_c.par_union(&set_d), (0..6).chain((6..9).rev()));
        check(set_d.par_union(&set_c), (3..9).rev().chain(0..3));
    }
}
