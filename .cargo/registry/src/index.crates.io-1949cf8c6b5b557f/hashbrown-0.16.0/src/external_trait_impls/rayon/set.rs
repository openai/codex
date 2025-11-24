//! Rayon extensions for `HashSet`.

use super::map;
use crate::hash_set::HashSet;
use crate::raw::{Allocator, Global};
use core::hash::{BuildHasher, Hash};
use rayon::iter::plumbing::UnindexedConsumer;
use rayon::iter::{FromParallelIterator, IntoParallelIterator, ParallelExtend, ParallelIterator};

/// Parallel iterator over elements of a consumed set.
///
/// This iterator is created by the [`into_par_iter`] method on [`HashSet`]
/// (provided by the [`IntoParallelIterator`] trait).
/// See its documentation for more.
///
/// [`into_par_iter`]: /hashbrown/struct.HashSet.html#method.into_par_iter
/// [`HashSet`]: /hashbrown/struct.HashSet.html
/// [`IntoParallelIterator`]: https://docs.rs/rayon/1.0/rayon/iter/trait.IntoParallelIterator.html
pub struct IntoParIter<T, A: Allocator = Global> {
    inner: map::IntoParIter<T, (), A>,
}

impl<T: Send, A: Allocator + Send> ParallelIterator for IntoParIter<T, A> {
    type Item = T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner.map(|(k, _)| k).drive_unindexed(consumer)
    }
}

/// Parallel draining iterator over entries of a set.
///
/// This iterator is created by the [`par_drain`] method on [`HashSet`].
/// See its documentation for more.
///
/// [`par_drain`]: /hashbrown/struct.HashSet.html#method.par_drain
/// [`HashSet`]: /hashbrown/struct.HashSet.html
pub struct ParDrain<'a, T, A: Allocator = Global> {
    inner: map::ParDrain<'a, T, (), A>,
}

impl<T: Send, A: Allocator + Send + Sync> ParallelIterator for ParDrain<'_, T, A> {
    type Item = T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner.map(|(k, _)| k).drive_unindexed(consumer)
    }
}

/// Parallel iterator over shared references to elements in a set.
///
/// This iterator is created by the [`par_iter`] method on [`HashSet`]
/// (provided by the [`IntoParallelRefIterator`] trait).
/// See its documentation for more.
///
/// [`par_iter`]: /hashbrown/struct.HashSet.html#method.par_iter
/// [`HashSet`]: /hashbrown/struct.HashSet.html
/// [`IntoParallelRefIterator`]: https://docs.rs/rayon/1.0/rayon/iter/trait.IntoParallelRefIterator.html
pub struct ParIter<'a, T> {
    inner: map::ParKeys<'a, T, ()>,
}

impl<'a, T: Sync> ParallelIterator for ParIter<'a, T> {
    type Item = &'a T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.inner.drive_unindexed(consumer)
    }
}

/// Parallel iterator over shared references to elements in the difference of
/// sets.
///
/// This iterator is created by the [`par_difference`] method on [`HashSet`].
/// See its documentation for more.
///
/// [`par_difference`]: /hashbrown/struct.HashSet.html#method.par_difference
/// [`HashSet`]: /hashbrown/struct.HashSet.html
pub struct ParDifference<'a, T, S, A: Allocator = Global> {
    a: &'a HashSet<T, S, A>,
    b: &'a HashSet<T, S, A>,
}

impl<'a, T, S, A> ParallelIterator for ParDifference<'a, T, S, A>
where
    T: Eq + Hash + Sync,
    S: BuildHasher + Sync,
    A: Allocator + Sync,
{
    type Item = &'a T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.a
            .into_par_iter()
            .filter(|&x| !self.b.contains(x))
            .drive_unindexed(consumer)
    }
}

/// Parallel iterator over shared references to elements in the symmetric
/// difference of sets.
///
/// This iterator is created by the [`par_symmetric_difference`] method on
/// [`HashSet`].
/// See its documentation for more.
///
/// [`par_symmetric_difference`]: /hashbrown/struct.HashSet.html#method.par_symmetric_difference
/// [`HashSet`]: /hashbrown/struct.HashSet.html
pub struct ParSymmetricDifference<'a, T, S, A: Allocator = Global> {
    a: &'a HashSet<T, S, A>,
    b: &'a HashSet<T, S, A>,
}

impl<'a, T, S, A> ParallelIterator for ParSymmetricDifference<'a, T, S, A>
where
    T: Eq + Hash + Sync,
    S: BuildHasher + Sync,
    A: Allocator + Sync,
{
    type Item = &'a T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.a
            .par_difference(self.b)
            .chain(self.b.par_difference(self.a))
            .drive_unindexed(consumer)
    }
}

/// Parallel iterator over shared references to elements in the intersection of
/// sets.
///
/// This iterator is created by the [`par_intersection`] method on [`HashSet`].
/// See its documentation for more.
///
/// [`par_intersection`]: /hashbrown/struct.HashSet.html#method.par_intersection
/// [`HashSet`]: /hashbrown/struct.HashSet.html
pub struct ParIntersection<'a, T, S, A: Allocator = Global> {
    a: &'a HashSet<T, S, A>,
    b: &'a HashSet<T, S, A>,
}

impl<'a, T, S, A> ParallelIterator for ParIntersection<'a, T, S, A>
where
    T: Eq + Hash + Sync,
    S: BuildHasher + Sync,
    A: Allocator + Sync,
{
    type Item = &'a T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        self.a
            .into_par_iter()
            .filter(|&x| self.b.contains(x))
            .drive_unindexed(consumer)
    }
}

/// Parallel iterator over shared references to elements in the union of sets.
///
/// This iterator is created by the [`par_union`] method on [`HashSet`].
/// See its documentation for more.
///
/// [`par_union`]: /hashbrown/struct.HashSet.html#method.par_union
/// [`HashSet`]: /hashbrown/struct.HashSet.html
pub struct ParUnion<'a, T, S, A: Allocator = Global> {
    a: &'a HashSet<T, S, A>,
    b: &'a HashSet<T, S, A>,
}

impl<'a, T, S, A> ParallelIterator for ParUnion<'a, T, S, A>
where
    T: Eq + Hash + Sync,
    S: BuildHasher + Sync,
    A: Allocator + Sync,
{
    type Item = &'a T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        // We'll iterate one set in full, and only the remaining difference from the other.
        // Use the smaller set for the difference in order to reduce hash lookups.
        let (smaller, larger) = if self.a.len() <= self.b.len() {
            (self.a, self.b)
        } else {
            (self.b, self.a)
        };
        larger
            .into_par_iter()
            .chain(smaller.par_difference(larger))
            .drive_unindexed(consumer)
    }
}

impl<T, S, A> HashSet<T, S, A>
where
    T: Eq + Hash + Sync,
    S: BuildHasher + Sync,
    A: Allocator + Sync,
{
    /// Visits (potentially in parallel) the values representing the union,
    /// i.e. all the values in `self` or `other`, without duplicates.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn par_union<'a>(&'a self, other: &'a Self) -> ParUnion<'a, T, S, A> {
        ParUnion { a: self, b: other }
    }

    /// Visits (potentially in parallel) the values representing the difference,
    /// i.e. the values that are in `self` but not in `other`.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn par_difference<'a>(&'a self, other: &'a Self) -> ParDifference<'a, T, S, A> {
        ParDifference { a: self, b: other }
    }

    /// Visits (potentially in parallel) the values representing the symmetric
    /// difference, i.e. the values that are in `self` or in `other` but not in both.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn par_symmetric_difference<'a>(
        &'a self,
        other: &'a Self,
    ) -> ParSymmetricDifference<'a, T, S, A> {
        ParSymmetricDifference { a: self, b: other }
    }

    /// Visits (potentially in parallel) the values representing the
    /// intersection, i.e. the values that are both in `self` and `other`.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn par_intersection<'a>(&'a self, other: &'a Self) -> ParIntersection<'a, T, S, A> {
        ParIntersection { a: self, b: other }
    }

    /// Returns `true` if `self` has no elements in common with `other`.
    /// This is equivalent to checking for an empty intersection.
    ///
    /// This method runs in a potentially parallel fashion.
    pub fn par_is_disjoint(&self, other: &Self) -> bool {
        self.into_par_iter().all(|x| !other.contains(x))
    }

    /// Returns `true` if the set is a subset of another,
    /// i.e. `other` contains at least all the values in `self`.
    ///
    /// This method runs in a potentially parallel fashion.
    pub fn par_is_subset(&self, other: &Self) -> bool {
        if self.len() <= other.len() {
            self.into_par_iter().all(|x| other.contains(x))
        } else {
            false
        }
    }

    /// Returns `true` if the set is a superset of another,
    /// i.e. `self` contains at least all the values in `other`.
    ///
    /// This method runs in a potentially parallel fashion.
    pub fn par_is_superset(&self, other: &Self) -> bool {
        other.par_is_subset(self)
    }

    /// Returns `true` if the set is equal to another,
    /// i.e. both sets contain the same values.
    ///
    /// This method runs in a potentially parallel fashion.
    pub fn par_eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.par_is_subset(other)
    }
}

impl<T, S, A> HashSet<T, S, A>
where
    T: Eq + Hash + Send,
    A: Allocator + Send,
{
    /// Consumes (potentially in parallel) all values in an arbitrary order,
    /// while preserving the set's allocated memory for reuse.
    #[cfg_attr(feature = "inline-more", inline)]
    pub fn par_drain(&mut self) -> ParDrain<'_, T, A> {
        ParDrain {
            inner: self.map.par_drain(),
        }
    }
}

impl<T: Send, S, A: Allocator + Send> IntoParallelIterator for HashSet<T, S, A> {
    type Item = T;
    type Iter = IntoParIter<T, A>;

    #[cfg_attr(feature = "inline-more", inline)]
    fn into_par_iter(self) -> Self::Iter {
        IntoParIter {
            inner: self.map.into_par_iter(),
        }
    }
}

impl<'a, T: Sync, S, A: Allocator> IntoParallelIterator for &'a HashSet<T, S, A> {
    type Item = &'a T;
    type Iter = ParIter<'a, T>;

    #[cfg_attr(feature = "inline-more", inline)]
    fn into_par_iter(self) -> Self::Iter {
        ParIter {
            inner: self.map.par_keys(),
        }
    }
}

/// Collect values from a parallel iterator into a hashset.
impl<T, S> FromParallelIterator<T> for HashSet<T, S, Global>
where
    T: Eq + Hash + Send,
    S: BuildHasher + Default,
{
    fn from_par_iter<P>(par_iter: P) -> Self
    where
        P: IntoParallelIterator<Item = T>,
    {
        let mut set = HashSet::default();
        set.par_extend(par_iter);
        set
    }
}

/// Extend a hash set with items from a parallel iterator.
impl<T, S> ParallelExtend<T> for HashSet<T, S, Global>
where
    T: Eq + Hash + Send,
    S: BuildHasher,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = T>,
    {
        extend(self, par_iter);
    }
}

/// Extend a hash set with copied items from a parallel iterator.
impl<'a, T, S> ParallelExtend<&'a T> for HashSet<T, S, Global>
where
    T: 'a + Copy + Eq + Hash + Sync,
    S: BuildHasher,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = &'a T>,
    {
        extend(self, par_iter);
    }
}

// This is equal to the normal `HashSet` -- no custom advantage.
fn extend<T, S, I, A>(set: &mut HashSet<T, S, A>, par_iter: I)
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
    I: IntoParallelIterator,
    HashSet<T, S, A>: Extend<I::Item>,
{
    let (list, len) = super::helpers::collect(par_iter);

    // Values may be already present or show multiple times in the iterator.
    // Reserve the entire length if the set is empty.
    // Otherwise reserve half the length (rounded up), so the set
    // will only resize twice in the worst case.
    let reserve = if set.is_empty() { len } else { (len + 1) / 2 };
    set.reserve(reserve);
    for vec in list {
        set.extend(vec);
    }
}

#[cfg(test)]
mod test_par_set {
    use alloc::vec::Vec;
    use core::sync::atomic::{AtomicUsize, Ordering};

    use rayon::prelude::*;

    use crate::hash_set::HashSet;

    #[test]
    fn test_disjoint() {
        let mut xs = HashSet::new();
        let mut ys = HashSet::new();
        assert!(xs.par_is_disjoint(&ys));
        assert!(ys.par_is_disjoint(&xs));
        assert!(xs.insert(5));
        assert!(ys.insert(11));
        assert!(xs.par_is_disjoint(&ys));
        assert!(ys.par_is_disjoint(&xs));
        assert!(xs.insert(7));
        assert!(xs.insert(19));
        assert!(xs.insert(4));
        assert!(ys.insert(2));
        assert!(ys.insert(-11));
        assert!(xs.par_is_disjoint(&ys));
        assert!(ys.par_is_disjoint(&xs));
        assert!(ys.insert(7));
        assert!(!xs.par_is_disjoint(&ys));
        assert!(!ys.par_is_disjoint(&xs));
    }

    #[test]
    fn test_subset_and_superset() {
        let mut a = HashSet::new();
        assert!(a.insert(0));
        assert!(a.insert(5));
        assert!(a.insert(11));
        assert!(a.insert(7));

        let mut b = HashSet::new();
        assert!(b.insert(0));
        assert!(b.insert(7));
        assert!(b.insert(19));
        assert!(b.insert(250));
        assert!(b.insert(11));
        assert!(b.insert(200));

        assert!(!a.par_is_subset(&b));
        assert!(!a.par_is_superset(&b));
        assert!(!b.par_is_subset(&a));
        assert!(!b.par_is_superset(&a));

        assert!(b.insert(5));

        assert!(a.par_is_subset(&b));
        assert!(!a.par_is_superset(&b));
        assert!(!b.par_is_subset(&a));
        assert!(b.par_is_superset(&a));
    }

    #[test]
    fn test_iterate() {
        let mut a = HashSet::new();
        for i in 0..32 {
            assert!(a.insert(i));
        }
        let observed = AtomicUsize::new(0);
        a.par_iter().for_each(|k| {
            observed.fetch_or(1 << *k, Ordering::Relaxed);
        });
        assert_eq!(observed.into_inner(), 0xFFFF_FFFF);
    }

    #[test]
    fn test_intersection() {
        let mut a = HashSet::new();
        let mut b = HashSet::new();

        assert!(a.insert(11));
        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(77));
        assert!(a.insert(103));
        assert!(a.insert(5));
        assert!(a.insert(-5));

        assert!(b.insert(2));
        assert!(b.insert(11));
        assert!(b.insert(77));
        assert!(b.insert(-9));
        assert!(b.insert(-42));
        assert!(b.insert(5));
        assert!(b.insert(3));

        let expected = [3, 5, 11, 77];
        let i = a
            .par_intersection(&b)
            .map(|x| {
                assert!(expected.contains(x));
                1
            })
            .sum::<usize>();
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_difference() {
        let mut a = HashSet::new();
        let mut b = HashSet::new();

        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(5));
        assert!(a.insert(9));
        assert!(a.insert(11));

        assert!(b.insert(3));
        assert!(b.insert(9));

        let expected = [1, 5, 11];
        let i = a
            .par_difference(&b)
            .map(|x| {
                assert!(expected.contains(x));
                1
            })
            .sum::<usize>();
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_symmetric_difference() {
        let mut a = HashSet::new();
        let mut b = HashSet::new();

        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(5));
        assert!(a.insert(9));
        assert!(a.insert(11));

        assert!(b.insert(-2));
        assert!(b.insert(3));
        assert!(b.insert(9));
        assert!(b.insert(14));
        assert!(b.insert(22));

        let expected = [-2, 1, 5, 11, 14, 22];
        let i = a
            .par_symmetric_difference(&b)
            .map(|x| {
                assert!(expected.contains(x));
                1
            })
            .sum::<usize>();
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_union() {
        let mut a = HashSet::new();
        let mut b = HashSet::new();

        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(5));
        assert!(a.insert(9));
        assert!(a.insert(11));
        assert!(a.insert(16));
        assert!(a.insert(19));
        assert!(a.insert(24));

        assert!(b.insert(-2));
        assert!(b.insert(1));
        assert!(b.insert(5));
        assert!(b.insert(9));
        assert!(b.insert(13));
        assert!(b.insert(19));

        let expected = [-2, 1, 3, 5, 9, 11, 13, 16, 19, 24];
        let i = a
            .par_union(&b)
            .map(|x| {
                assert!(expected.contains(x));
                1
            })
            .sum::<usize>();
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_from_iter() {
        let xs = [1, 2, 3, 4, 5, 6, 7, 8, 9];

        let set: HashSet<_> = xs.par_iter().cloned().collect();

        for x in &xs {
            assert!(set.contains(x));
        }
    }

    #[test]
    fn test_move_iter() {
        let hs = {
            let mut hs = HashSet::new();

            hs.insert('a');
            hs.insert('b');

            hs
        };

        let v = hs.into_par_iter().collect::<Vec<char>>();
        assert!(v == ['a', 'b'] || v == ['b', 'a']);
    }

    #[test]
    fn test_eq() {
        // These constants once happened to expose a bug in insert().
        // I'm keeping them around to prevent a regression.
        let mut s1 = HashSet::new();

        s1.insert(1);
        s1.insert(2);
        s1.insert(3);

        let mut s2 = HashSet::new();

        s2.insert(1);
        s2.insert(2);

        assert!(!s1.par_eq(&s2));

        s2.insert(3);

        assert!(s1.par_eq(&s2));
    }

    #[test]
    fn test_extend_ref() {
        let mut a = HashSet::new();
        a.insert(1);

        a.par_extend(&[2, 3, 4][..]);

        assert_eq!(a.len(), 4);
        assert!(a.contains(&1));
        assert!(a.contains(&2));
        assert!(a.contains(&3));
        assert!(a.contains(&4));

        let mut b = HashSet::new();
        b.insert(5);
        b.insert(6);

        a.par_extend(&b);

        assert_eq!(a.len(), 6);
        assert!(a.contains(&1));
        assert!(a.contains(&2));
        assert!(a.contains(&3));
        assert!(a.contains(&4));
        assert!(a.contains(&5));
        assert!(a.contains(&6));
    }
}
