use crate::map::{ExtractCore, IndexMapCore};

use super::{Bucket, IndexSet, Slice};

use alloc::vec::{self, Vec};
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::iter::{Chain, FusedIterator};
use core::ops::RangeBounds;
use core::slice::Iter as SliceIter;

impl<'a, T, S> IntoIterator for &'a IndexSet<T, S> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T, S> IntoIterator for IndexSet<T, S> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self.into_entries())
    }
}

/// An iterator over the items of an [`IndexSet`].
///
/// This `struct` is created by the [`IndexSet::iter`] method.
/// See its documentation for more.
pub struct Iter<'a, T> {
    iter: SliceIter<'a, Bucket<T>>,
}

impl<'a, T> Iter<'a, T> {
    pub(super) fn new(entries: &'a [Bucket<T>]) -> Self {
        Self {
            iter: entries.iter(),
        }
    }

    /// Returns a slice of the remaining entries in the iterator.
    pub fn as_slice(&self) -> &'a Slice<T> {
        Slice::from_slice(self.iter.as_slice())
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    iterator_methods!(Bucket::key_ref);
}

impl<T> DoubleEndedIterator for Iter<'_, T> {
    double_ended_iterator_methods!(Bucket::key_ref);
}

impl<T> ExactSizeIterator for Iter<'_, T> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<T> FusedIterator for Iter<'_, T> {}

impl<T> Clone for Iter<'_, T> {
    fn clone(&self) -> Self {
        Iter {
            iter: self.iter.clone(),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Iter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

impl<T> Default for Iter<'_, T> {
    fn default() -> Self {
        Self { iter: [].iter() }
    }
}

/// An owning iterator over the items of an [`IndexSet`].
///
/// This `struct` is created by the [`IndexSet::into_iter`] method
/// (provided by the [`IntoIterator`] trait). See its documentation for more.
#[derive(Clone)]
pub struct IntoIter<T> {
    iter: vec::IntoIter<Bucket<T>>,
}

impl<T> IntoIter<T> {
    pub(super) fn new(entries: Vec<Bucket<T>>) -> Self {
        Self {
            iter: entries.into_iter(),
        }
    }

    /// Returns a slice of the remaining entries in the iterator.
    pub fn as_slice(&self) -> &Slice<T> {
        Slice::from_slice(self.iter.as_slice())
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    iterator_methods!(Bucket::key);
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    double_ended_iterator_methods!(Bucket::key);
}

impl<T> ExactSizeIterator for IntoIter<T> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<T> FusedIterator for IntoIter<T> {}

impl<T: fmt::Debug> fmt::Debug for IntoIter<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.iter.as_slice().iter().map(Bucket::key_ref);
        f.debug_list().entries(iter).finish()
    }
}

impl<T> Default for IntoIter<T> {
    fn default() -> Self {
        Self {
            iter: Vec::new().into_iter(),
        }
    }
}

/// A draining iterator over the items of an [`IndexSet`].
///
/// This `struct` is created by the [`IndexSet::drain`] method.
/// See its documentation for more.
pub struct Drain<'a, T> {
    iter: vec::Drain<'a, Bucket<T>>,
}

impl<'a, T> Drain<'a, T> {
    pub(super) fn new(iter: vec::Drain<'a, Bucket<T>>) -> Self {
        Self { iter }
    }

    /// Returns a slice of the remaining entries in the iterator.
    pub fn as_slice(&self) -> &Slice<T> {
        Slice::from_slice(self.iter.as_slice())
    }
}

impl<T> Iterator for Drain<'_, T> {
    type Item = T;

    iterator_methods!(Bucket::key);
}

impl<T> DoubleEndedIterator for Drain<'_, T> {
    double_ended_iterator_methods!(Bucket::key);
}

impl<T> ExactSizeIterator for Drain<'_, T> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<T> FusedIterator for Drain<'_, T> {}

impl<T: fmt::Debug> fmt::Debug for Drain<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.iter.as_slice().iter().map(Bucket::key_ref);
        f.debug_list().entries(iter).finish()
    }
}

/// A lazy iterator producing elements in the difference of [`IndexSet`]s.
///
/// This `struct` is created by the [`IndexSet::difference`] method.
/// See its documentation for more.
pub struct Difference<'a, T, S> {
    iter: Iter<'a, T>,
    other: &'a IndexSet<T, S>,
}

impl<'a, T, S> Difference<'a, T, S> {
    pub(super) fn new<S1>(set: &'a IndexSet<T, S1>, other: &'a IndexSet<T, S>) -> Self {
        Self {
            iter: set.iter(),
            other,
        }
    }
}

impl<'a, T, S> Iterator for Difference<'a, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.iter.next() {
            if !self.other.contains(item) {
                return Some(item);
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }
}

impl<T, S> DoubleEndedIterator for Difference<'_, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.iter.next_back() {
            if !self.other.contains(item) {
                return Some(item);
            }
        }
        None
    }
}

impl<T, S> FusedIterator for Difference<'_, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
}

impl<T, S> Clone for Difference<'_, T, S> {
    fn clone(&self) -> Self {
        Difference {
            iter: self.iter.clone(),
            ..*self
        }
    }
}

impl<T, S> fmt::Debug for Difference<'_, T, S>
where
    T: fmt::Debug + Eq + Hash,
    S: BuildHasher,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// A lazy iterator producing elements in the intersection of [`IndexSet`]s.
///
/// This `struct` is created by the [`IndexSet::intersection`] method.
/// See its documentation for more.
pub struct Intersection<'a, T, S> {
    iter: Iter<'a, T>,
    other: &'a IndexSet<T, S>,
}

impl<'a, T, S> Intersection<'a, T, S> {
    pub(super) fn new<S1>(set: &'a IndexSet<T, S1>, other: &'a IndexSet<T, S>) -> Self {
        Self {
            iter: set.iter(),
            other,
        }
    }
}

impl<'a, T, S> Iterator for Intersection<'a, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.iter.next() {
            if self.other.contains(item) {
                return Some(item);
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }
}

impl<T, S> DoubleEndedIterator for Intersection<'_, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.iter.next_back() {
            if self.other.contains(item) {
                return Some(item);
            }
        }
        None
    }
}

impl<T, S> FusedIterator for Intersection<'_, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
}

impl<T, S> Clone for Intersection<'_, T, S> {
    fn clone(&self) -> Self {
        Intersection {
            iter: self.iter.clone(),
            ..*self
        }
    }
}

impl<T, S> fmt::Debug for Intersection<'_, T, S>
where
    T: fmt::Debug + Eq + Hash,
    S: BuildHasher,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// A lazy iterator producing elements in the symmetric difference of [`IndexSet`]s.
///
/// This `struct` is created by the [`IndexSet::symmetric_difference`] method.
/// See its documentation for more.
pub struct SymmetricDifference<'a, T, S1, S2> {
    iter: Chain<Difference<'a, T, S2>, Difference<'a, T, S1>>,
}

impl<'a, T, S1, S2> SymmetricDifference<'a, T, S1, S2>
where
    T: Eq + Hash,
    S1: BuildHasher,
    S2: BuildHasher,
{
    pub(super) fn new(set1: &'a IndexSet<T, S1>, set2: &'a IndexSet<T, S2>) -> Self {
        let diff1 = set1.difference(set2);
        let diff2 = set2.difference(set1);
        Self {
            iter: diff1.chain(diff2),
        }
    }
}

impl<'a, T, S1, S2> Iterator for SymmetricDifference<'a, T, S1, S2>
where
    T: Eq + Hash,
    S1: BuildHasher,
    S2: BuildHasher,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn fold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, f)
    }
}

impl<T, S1, S2> DoubleEndedIterator for SymmetricDifference<'_, T, S1, S2>
where
    T: Eq + Hash,
    S1: BuildHasher,
    S2: BuildHasher,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }

    fn rfold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.rfold(init, f)
    }
}

impl<T, S1, S2> FusedIterator for SymmetricDifference<'_, T, S1, S2>
where
    T: Eq + Hash,
    S1: BuildHasher,
    S2: BuildHasher,
{
}

impl<T, S1, S2> Clone for SymmetricDifference<'_, T, S1, S2> {
    fn clone(&self) -> Self {
        SymmetricDifference {
            iter: self.iter.clone(),
        }
    }
}

impl<T, S1, S2> fmt::Debug for SymmetricDifference<'_, T, S1, S2>
where
    T: fmt::Debug + Eq + Hash,
    S1: BuildHasher,
    S2: BuildHasher,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// A lazy iterator producing elements in the union of [`IndexSet`]s.
///
/// This `struct` is created by the [`IndexSet::union`] method.
/// See its documentation for more.
pub struct Union<'a, T, S> {
    iter: Chain<Iter<'a, T>, Difference<'a, T, S>>,
}

impl<'a, T, S> Union<'a, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    pub(super) fn new<S2>(set1: &'a IndexSet<T, S>, set2: &'a IndexSet<T, S2>) -> Self
    where
        S2: BuildHasher,
    {
        Self {
            iter: set1.iter().chain(set2.difference(set1)),
        }
    }
}

impl<'a, T, S> Iterator for Union<'a, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn fold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, f)
    }
}

impl<T, S> DoubleEndedIterator for Union<'_, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.next_back()
    }

    fn rfold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.rfold(init, f)
    }
}

impl<T, S> FusedIterator for Union<'_, T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
}

impl<T, S> Clone for Union<'_, T, S> {
    fn clone(&self) -> Self {
        Union {
            iter: self.iter.clone(),
        }
    }
}

impl<T, S> fmt::Debug for Union<'_, T, S>
where
    T: fmt::Debug + Eq + Hash,
    S: BuildHasher,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// A splicing iterator for `IndexSet`.
///
/// This `struct` is created by [`IndexSet::splice()`].
/// See its documentation for more.
pub struct Splice<'a, I, T, S>
where
    I: Iterator<Item = T>,
    T: Hash + Eq,
    S: BuildHasher,
{
    iter: crate::map::Splice<'a, UnitValue<I>, T, (), S>,
}

impl<'a, I, T, S> Splice<'a, I, T, S>
where
    I: Iterator<Item = T>,
    T: Hash + Eq,
    S: BuildHasher,
{
    #[track_caller]
    pub(super) fn new<R>(set: &'a mut IndexSet<T, S>, range: R, replace_with: I) -> Self
    where
        R: RangeBounds<usize>,
    {
        Self {
            iter: set.map.splice(range, UnitValue(replace_with)),
        }
    }
}

impl<I, T, S> Iterator for Splice<'_, I, T, S>
where
    I: Iterator<Item = T>,
    T: Hash + Eq,
    S: BuildHasher,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.iter.next()?.0)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<I, T, S> DoubleEndedIterator for Splice<'_, I, T, S>
where
    I: Iterator<Item = T>,
    T: Hash + Eq,
    S: BuildHasher,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        Some(self.iter.next_back()?.0)
    }
}

impl<I, T, S> ExactSizeIterator for Splice<'_, I, T, S>
where
    I: Iterator<Item = T>,
    T: Hash + Eq,
    S: BuildHasher,
{
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<I, T, S> FusedIterator for Splice<'_, I, T, S>
where
    I: Iterator<Item = T>,
    T: Hash + Eq,
    S: BuildHasher,
{
}

struct UnitValue<I>(I);

impl<I: Iterator> Iterator for UnitValue<I> {
    type Item = (I::Item, ());

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|x| (x, ()))
    }
}

impl<I, T, S> fmt::Debug for Splice<'_, I, T, S>
where
    I: fmt::Debug + Iterator<Item = T>,
    T: fmt::Debug + Hash + Eq,
    S: BuildHasher,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.iter, f)
    }
}

impl<I: fmt::Debug> fmt::Debug for UnitValue<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

/// An extracting iterator for `IndexSet`.
///
/// This `struct` is created by [`IndexSet::extract_if()`].
/// See its documentation for more.
pub struct ExtractIf<'a, T, F> {
    inner: ExtractCore<'a, T, ()>,
    pred: F,
}

impl<T, F> ExtractIf<'_, T, F> {
    #[track_caller]
    pub(super) fn new<R>(core: &mut IndexMapCore<T, ()>, range: R, pred: F) -> ExtractIf<'_, T, F>
    where
        R: RangeBounds<usize>,
        F: FnMut(&T) -> bool,
    {
        ExtractIf {
            inner: core.extract(range),
            pred,
        }
    }
}

impl<T, F> Iterator for ExtractIf<'_, T, F>
where
    F: FnMut(&T) -> bool,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .extract_if(|bucket| (self.pred)(bucket.key_ref()))
            .map(Bucket::key)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.inner.remaining()))
    }
}

impl<T, F> FusedIterator for ExtractIf<'_, T, F> where F: FnMut(&T) -> bool {}

impl<T, F> fmt::Debug for ExtractIf<'_, T, F>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtractIf").finish_non_exhaustive()
    }
}
