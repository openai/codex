//! A hash set implemented using [`IndexMap`]

mod iter;
mod mutable;
mod slice;

#[cfg(test)]
mod tests;

pub use self::iter::{
    Difference, Drain, ExtractIf, Intersection, IntoIter, Iter, Splice, SymmetricDifference, Union,
};
pub use self::mutable::MutableValues;
pub use self::slice::Slice;

#[cfg(feature = "rayon")]
pub use crate::rayon::set as rayon;
use crate::TryReserveError;

#[cfg(feature = "std")]
use std::hash::RandomState;

use crate::util::try_simplify_range;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::ops::{BitAnd, BitOr, BitXor, Index, RangeBounds, Sub};

use super::{Equivalent, IndexMap};

type Bucket<T> = super::Bucket<T, ()>;

/// A hash set where the iteration order of the values is independent of their
/// hash values.
///
/// The interface is closely compatible with the standard
/// [`HashSet`][std::collections::HashSet],
/// but also has additional features.
///
/// # Order
///
/// The values have a consistent order that is determined by the sequence of
/// insertion and removal calls on the set. The order does not depend on the
/// values or the hash function at all. Note that insertion order and value
/// are not affected if a re-insertion is attempted once an element is
/// already present.
///
/// All iterators traverse the set *in order*.  Set operation iterators like
/// [`IndexSet::union`] produce a concatenated order, as do their matching "bitwise"
/// operators.  See their documentation for specifics.
///
/// The insertion order is preserved, with **notable exceptions** like the
/// [`.remove()`][Self::remove] or [`.swap_remove()`][Self::swap_remove] methods.
/// Methods such as [`.sort_by()`][Self::sort_by] of
/// course result in a new order, depending on the sorting order.
///
/// # Indices
///
/// The values are indexed in a compact range without holes in the range
/// `0..self.len()`. For example, the method `.get_full` looks up the index for
/// a value, and the method `.get_index` looks up the value by index.
///
/// # Complexity
///
/// Internally, `IndexSet<T, S>` just holds an [`IndexMap<T, (), S>`](IndexMap). Thus the complexity
/// of the two are the same for most methods.
///
/// # Examples
///
/// ```
/// use indexmap::IndexSet;
///
/// // Collects which letters appear in a sentence.
/// let letters: IndexSet<_> = "a short treatise on fungi".chars().collect();
///
/// assert!(letters.contains(&'s'));
/// assert!(letters.contains(&'t'));
/// assert!(letters.contains(&'u'));
/// assert!(!letters.contains(&'y'));
/// ```
#[cfg(feature = "std")]
pub struct IndexSet<T, S = RandomState> {
    pub(crate) map: IndexMap<T, (), S>,
}
#[cfg(not(feature = "std"))]
pub struct IndexSet<T, S> {
    pub(crate) map: IndexMap<T, (), S>,
}

impl<T, S> Clone for IndexSet<T, S>
where
    T: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        IndexSet {
            map: self.map.clone(),
        }
    }

    fn clone_from(&mut self, other: &Self) {
        self.map.clone_from(&other.map);
    }
}

impl<T, S> fmt::Debug for IndexSet<T, S>
where
    T: fmt::Debug,
{
    #[cfg(not(feature = "test_debug"))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }

    #[cfg(feature = "test_debug")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Let the inner `IndexMap` print all of its details
        f.debug_struct("IndexSet").field("map", &self.map).finish()
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<T> IndexSet<T> {
    /// Create a new set. (Does not allocate.)
    pub fn new() -> Self {
        IndexSet {
            map: IndexMap::new(),
        }
    }

    /// Create a new set with capacity for `n` elements.
    /// (Does not allocate if `n` is zero.)
    ///
    /// Computes in **O(n)** time.
    pub fn with_capacity(n: usize) -> Self {
        IndexSet {
            map: IndexMap::with_capacity(n),
        }
    }
}

impl<T, S> IndexSet<T, S> {
    /// Create a new set with capacity for `n` elements.
    /// (Does not allocate if `n` is zero.)
    ///
    /// Computes in **O(n)** time.
    pub fn with_capacity_and_hasher(n: usize, hash_builder: S) -> Self {
        IndexSet {
            map: IndexMap::with_capacity_and_hasher(n, hash_builder),
        }
    }

    /// Create a new set with `hash_builder`.
    ///
    /// This function is `const`, so it
    /// can be called in `static` contexts.
    pub const fn with_hasher(hash_builder: S) -> Self {
        IndexSet {
            map: IndexMap::with_hasher(hash_builder),
        }
    }

    #[inline]
    pub(crate) fn into_entries(self) -> Vec<Bucket<T>> {
        self.map.into_entries()
    }

    #[inline]
    pub(crate) fn as_entries(&self) -> &[Bucket<T>] {
        self.map.as_entries()
    }

    pub(crate) fn with_entries<F>(&mut self, f: F)
    where
        F: FnOnce(&mut [Bucket<T>]),
    {
        self.map.with_entries(f);
    }

    /// Return the number of elements the set can hold without reallocating.
    ///
    /// This number is a lower bound; the set might be able to hold more,
    /// but is guaranteed to be able to hold at least this many.
    ///
    /// Computes in **O(1)** time.
    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }

    /// Return a reference to the set's `BuildHasher`.
    pub fn hasher(&self) -> &S {
        self.map.hasher()
    }

    /// Return the number of elements in the set.
    ///
    /// Computes in **O(1)** time.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns true if the set contains no elements.
    ///
    /// Computes in **O(1)** time.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Return an iterator over the values of the set, in their order
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(self.as_entries())
    }

    /// Remove all elements in the set, while preserving its capacity.
    ///
    /// Computes in **O(n)** time.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Shortens the set, keeping the first `len` elements and dropping the rest.
    ///
    /// If `len` is greater than the set's current length, this has no effect.
    pub fn truncate(&mut self, len: usize) {
        self.map.truncate(len);
    }

    /// Clears the `IndexSet` in the given index range, returning those values
    /// as a drain iterator.
    ///
    /// The range may be any type that implements [`RangeBounds<usize>`],
    /// including all of the `std::ops::Range*` types, or even a tuple pair of
    /// `Bound` start and end values. To drain the set entirely, use `RangeFull`
    /// like `set.drain(..)`.
    ///
    /// This shifts down all entries following the drained range to fill the
    /// gap, and keeps the allocated memory for reuse.
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the set.
    #[track_caller]
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, T>
    where
        R: RangeBounds<usize>,
    {
        Drain::new(self.map.core.drain(range))
    }

    /// Creates an iterator which uses a closure to determine if a value should be removed,
    /// for all values in the given range.
    ///
    /// If the closure returns true, then the value is removed and yielded.
    /// If the closure returns false, the value will remain in the list and will not be yielded
    /// by the iterator.
    ///
    /// The range may be any type that implements [`RangeBounds<usize>`],
    /// including all of the `std::ops::Range*` types, or even a tuple pair of
    /// `Bound` start and end values. To check the entire set, use `RangeFull`
    /// like `set.extract_if(.., predicate)`.
    ///
    /// If the returned `ExtractIf` is not exhausted, e.g. because it is dropped without iterating
    /// or the iteration short-circuits, then the remaining elements will be retained.
    /// Use [`retain`] with a negated predicate if you do not need the returned iterator.
    ///
    /// [`retain`]: IndexSet::retain
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the set.
    ///
    /// # Examples
    ///
    /// Splitting a set into even and odd values, reusing the original set:
    ///
    /// ```
    /// use indexmap::IndexSet;
    ///
    /// let mut set: IndexSet<i32> = (0..8).collect();
    /// let extracted: IndexSet<i32> = set.extract_if(.., |v| v % 2 == 0).collect();
    ///
    /// let evens = extracted.into_iter().collect::<Vec<_>>();
    /// let odds = set.into_iter().collect::<Vec<_>>();
    ///
    /// assert_eq!(evens, vec![0, 2, 4, 6]);
    /// assert_eq!(odds, vec![1, 3, 5, 7]);
    /// ```
    #[track_caller]
    pub fn extract_if<F, R>(&mut self, range: R, pred: F) -> ExtractIf<'_, T, F>
    where
        F: FnMut(&T) -> bool,
        R: RangeBounds<usize>,
    {
        ExtractIf::new(&mut self.map.core, range, pred)
    }

    /// Splits the collection into two at the given index.
    ///
    /// Returns a newly allocated set containing the elements in the range
    /// `[at, len)`. After the call, the original set will be left containing
    /// the elements `[0, at)` with its previous capacity unchanged.
    ///
    /// ***Panics*** if `at > len`.
    #[track_caller]
    pub fn split_off(&mut self, at: usize) -> Self
    where
        S: Clone,
    {
        Self {
            map: self.map.split_off(at),
        }
    }

    /// Reserve capacity for `additional` more values.
    ///
    /// Computes in **O(n)** time.
    pub fn reserve(&mut self, additional: usize) {
        self.map.reserve(additional);
    }

    /// Reserve capacity for `additional` more values, without over-allocating.
    ///
    /// Unlike `reserve`, this does not deliberately over-allocate the entry capacity to avoid
    /// frequent re-allocations. However, the underlying data structures may still have internal
    /// capacity requirements, and the allocator itself may give more space than requested, so this
    /// cannot be relied upon to be precisely minimal.
    ///
    /// Computes in **O(n)** time.
    pub fn reserve_exact(&mut self, additional: usize) {
        self.map.reserve_exact(additional);
    }

    /// Try to reserve capacity for `additional` more values.
    ///
    /// Computes in **O(n)** time.
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.map.try_reserve(additional)
    }

    /// Try to reserve capacity for `additional` more values, without over-allocating.
    ///
    /// Unlike `try_reserve`, this does not deliberately over-allocate the entry capacity to avoid
    /// frequent re-allocations. However, the underlying data structures may still have internal
    /// capacity requirements, and the allocator itself may give more space than requested, so this
    /// cannot be relied upon to be precisely minimal.
    ///
    /// Computes in **O(n)** time.
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.map.try_reserve_exact(additional)
    }

    /// Shrink the capacity of the set as much as possible.
    ///
    /// Computes in **O(n)** time.
    pub fn shrink_to_fit(&mut self) {
        self.map.shrink_to_fit();
    }

    /// Shrink the capacity of the set with a lower limit.
    ///
    /// Computes in **O(n)** time.
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.map.shrink_to(min_capacity);
    }
}

impl<T, S> IndexSet<T, S>
where
    T: Hash + Eq,
    S: BuildHasher,
{
    /// Insert the value into the set.
    ///
    /// If an equivalent item already exists in the set, it returns
    /// `false` leaving the original value in the set and without
    /// altering its insertion order. Otherwise, it inserts the new
    /// item and returns `true`.
    ///
    /// Computes in **O(1)** time (amortized average).
    pub fn insert(&mut self, value: T) -> bool {
        self.map.insert(value, ()).is_none()
    }

    /// Insert the value into the set, and get its index.
    ///
    /// If an equivalent item already exists in the set, it returns
    /// the index of the existing item and `false`, leaving the
    /// original value in the set and without altering its insertion
    /// order. Otherwise, it inserts the new item and returns the index
    /// of the inserted item and `true`.
    ///
    /// Computes in **O(1)** time (amortized average).
    pub fn insert_full(&mut self, value: T) -> (usize, bool) {
        let (index, existing) = self.map.insert_full(value, ());
        (index, existing.is_none())
    }

    /// Insert the value into the set at its ordered position among sorted values.
    ///
    /// This is equivalent to finding the position with
    /// [`binary_search`][Self::binary_search], and if needed calling
    /// [`insert_before`][Self::insert_before] for a new value.
    ///
    /// If the sorted item is found in the set, it returns the index of that
    /// existing item and `false`, without any change. Otherwise, it inserts the
    /// new item and returns its sorted index and `true`.
    ///
    /// If the existing items are **not** already sorted, then the insertion
    /// index is unspecified (like [`slice::binary_search`]), but the value
    /// is moved to or inserted at that position regardless.
    ///
    /// Computes in **O(n)** time (average). Instead of repeating calls to
    /// `insert_sorted`, it may be faster to call batched [`insert`][Self::insert]
    /// or [`extend`][Self::extend] and only call [`sort`][Self::sort] or
    /// [`sort_unstable`][Self::sort_unstable] once.
    pub fn insert_sorted(&mut self, value: T) -> (usize, bool)
    where
        T: Ord,
    {
        let (index, existing) = self.map.insert_sorted(value, ());
        (index, existing.is_none())
    }

    /// Insert the value into the set at its ordered position among values
    /// sorted by `cmp`.
    ///
    /// This is equivalent to finding the position with
    /// [`binary_search_by`][Self::binary_search_by], then calling
    /// [`insert_before`][Self::insert_before].
    ///
    /// If the existing items are **not** already sorted, then the insertion
    /// index is unspecified (like [`slice::binary_search`]), but the value
    /// is moved to or inserted at that position regardless.
    ///
    /// Computes in **O(n)** time (average).
    pub fn insert_sorted_by<F>(&mut self, value: T, mut cmp: F) -> (usize, bool)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        let (index, existing) = self
            .map
            .insert_sorted_by(value, (), |a, (), b, ()| cmp(a, b));
        (index, existing.is_none())
    }

    /// Insert the value into the set at its ordered position among values
    /// using a sort-key extraction function.
    ///
    /// This is equivalent to finding the position with
    /// [`binary_search_by_key`][Self::binary_search_by_key] with `sort_key(key)`,
    /// then calling [`insert_before`][Self::insert_before].
    ///
    /// If the existing items are **not** already sorted, then the insertion
    /// index is unspecified (like [`slice::binary_search`]), but the value
    /// is moved to or inserted at that position regardless.
    ///
    /// Computes in **O(n)** time (average).
    pub fn insert_sorted_by_key<B, F>(&mut self, value: T, mut sort_key: F) -> (usize, bool)
    where
        B: Ord,
        F: FnMut(&T) -> B,
    {
        let (index, existing) = self.map.insert_sorted_by_key(value, (), |k, _| sort_key(k));
        (index, existing.is_none())
    }

    /// Insert the value into the set before the value at the given index, or at the end.
    ///
    /// If an equivalent item already exists in the set, it returns `false` leaving the
    /// original value in the set, but moved to the new position. The returned index
    /// will either be the given index or one less, depending on how the value moved.
    /// (See [`shift_insert`](Self::shift_insert) for different behavior here.)
    ///
    /// Otherwise, it inserts the new value exactly at the given index and returns `true`.
    ///
    /// ***Panics*** if `index` is out of bounds.
    /// Valid indices are `0..=set.len()` (inclusive).
    ///
    /// Computes in **O(n)** time (average).
    ///
    /// # Examples
    ///
    /// ```
    /// use indexmap::IndexSet;
    /// let mut set: IndexSet<char> = ('a'..='z').collect();
    ///
    /// // The new value '*' goes exactly at the given index.
    /// assert_eq!(set.get_index_of(&'*'), None);
    /// assert_eq!(set.insert_before(10, '*'), (10, true));
    /// assert_eq!(set.get_index_of(&'*'), Some(10));
    ///
    /// // Moving the value 'a' up will shift others down, so this moves *before* 10 to index 9.
    /// assert_eq!(set.insert_before(10, 'a'), (9, false));
    /// assert_eq!(set.get_index_of(&'a'), Some(9));
    /// assert_eq!(set.get_index_of(&'*'), Some(10));
    ///
    /// // Moving the value 'z' down will shift others up, so this moves to exactly 10.
    /// assert_eq!(set.insert_before(10, 'z'), (10, false));
    /// assert_eq!(set.get_index_of(&'z'), Some(10));
    /// assert_eq!(set.get_index_of(&'*'), Some(11));
    ///
    /// // Moving or inserting before the endpoint is also valid.
    /// assert_eq!(set.len(), 27);
    /// assert_eq!(set.insert_before(set.len(), '*'), (26, false));
    /// assert_eq!(set.get_index_of(&'*'), Some(26));
    /// assert_eq!(set.insert_before(set.len(), '+'), (27, true));
    /// assert_eq!(set.get_index_of(&'+'), Some(27));
    /// assert_eq!(set.len(), 28);
    /// ```
    #[track_caller]
    pub fn insert_before(&mut self, index: usize, value: T) -> (usize, bool) {
        let (index, existing) = self.map.insert_before(index, value, ());
        (index, existing.is_none())
    }

    /// Insert the value into the set at the given index.
    ///
    /// If an equivalent item already exists in the set, it returns `false` leaving
    /// the original value in the set, but moved to the given index.
    /// Note that existing values **cannot** be moved to `index == set.len()`!
    /// (See [`insert_before`](Self::insert_before) for different behavior here.)
    ///
    /// Otherwise, it inserts the new value at the given index and returns `true`.
    ///
    /// ***Panics*** if `index` is out of bounds.
    /// Valid indices are `0..set.len()` (exclusive) when moving an existing value, or
    /// `0..=set.len()` (inclusive) when inserting a new value.
    ///
    /// Computes in **O(n)** time (average).
    ///
    /// # Examples
    ///
    /// ```
    /// use indexmap::IndexSet;
    /// let mut set: IndexSet<char> = ('a'..='z').collect();
    ///
    /// // The new value '*' goes exactly at the given index.
    /// assert_eq!(set.get_index_of(&'*'), None);
    /// assert_eq!(set.shift_insert(10, '*'), true);
    /// assert_eq!(set.get_index_of(&'*'), Some(10));
    ///
    /// // Moving the value 'a' up to 10 will shift others down, including the '*' that was at 10.
    /// assert_eq!(set.shift_insert(10, 'a'), false);
    /// assert_eq!(set.get_index_of(&'a'), Some(10));
    /// assert_eq!(set.get_index_of(&'*'), Some(9));
    ///
    /// // Moving the value 'z' down to 9 will shift others up, including the '*' that was at 9.
    /// assert_eq!(set.shift_insert(9, 'z'), false);
    /// assert_eq!(set.get_index_of(&'z'), Some(9));
    /// assert_eq!(set.get_index_of(&'*'), Some(10));
    ///
    /// // Existing values can move to len-1 at most, but new values can insert at the endpoint.
    /// assert_eq!(set.len(), 27);
    /// assert_eq!(set.shift_insert(set.len() - 1, '*'), false);
    /// assert_eq!(set.get_index_of(&'*'), Some(26));
    /// assert_eq!(set.shift_insert(set.len(), '+'), true);
    /// assert_eq!(set.get_index_of(&'+'), Some(27));
    /// assert_eq!(set.len(), 28);
    /// ```
    ///
    /// ```should_panic
    /// use indexmap::IndexSet;
    /// let mut set: IndexSet<char> = ('a'..='z').collect();
    ///
    /// // This is an invalid index for moving an existing value!
    /// set.shift_insert(set.len(), 'a');
    /// ```
    #[track_caller]
    pub fn shift_insert(&mut self, index: usize, value: T) -> bool {
        self.map.shift_insert(index, value, ()).is_none()
    }

    /// Adds a value to the set, replacing the existing value, if any, that is
    /// equal to the given one, without altering its insertion order. Returns
    /// the replaced value.
    ///
    /// Computes in **O(1)** time (average).
    pub fn replace(&mut self, value: T) -> Option<T> {
        self.replace_full(value).1
    }

    /// Adds a value to the set, replacing the existing value, if any, that is
    /// equal to the given one, without altering its insertion order. Returns
    /// the index of the item and its replaced value.
    ///
    /// Computes in **O(1)** time (average).
    pub fn replace_full(&mut self, value: T) -> (usize, Option<T>) {
        let hash = self.map.hash(&value);
        match self.map.core.replace_full(hash, value, ()) {
            (i, Some((replaced, ()))) => (i, Some(replaced)),
            (i, None) => (i, None),
        }
    }

    /// Replaces the value at the given index. The new value does not need to be
    /// equivalent to the one it is replacing, but it must be unique to the rest
    /// of the set.
    ///
    /// Returns `Ok(old_value)` if successful, or `Err((other_index, value))` if
    /// an equivalent value already exists at a different index. The set will be
    /// unchanged in the error case.
    ///
    /// ***Panics*** if `index` is out of bounds.
    ///
    /// Computes in **O(1)** time (average).
    #[track_caller]
    pub fn replace_index(&mut self, index: usize, value: T) -> Result<T, (usize, T)> {
        self.map.replace_index(index, value)
    }

    /// Return an iterator over the values that are in `self` but not `other`.
    ///
    /// Values are produced in the same order that they appear in `self`.
    pub fn difference<'a, S2>(&'a self, other: &'a IndexSet<T, S2>) -> Difference<'a, T, S2>
    where
        S2: BuildHasher,
    {
        Difference::new(self, other)
    }

    /// Return an iterator over the values that are in `self` or `other`,
    /// but not in both.
    ///
    /// Values from `self` are produced in their original order, followed by
    /// values from `other` in their original order.
    pub fn symmetric_difference<'a, S2>(
        &'a self,
        other: &'a IndexSet<T, S2>,
    ) -> SymmetricDifference<'a, T, S, S2>
    where
        S2: BuildHasher,
    {
        SymmetricDifference::new(self, other)
    }

    /// Return an iterator over the values that are in both `self` and `other`.
    ///
    /// Values are produced in the same order that they appear in `self`.
    pub fn intersection<'a, S2>(&'a self, other: &'a IndexSet<T, S2>) -> Intersection<'a, T, S2>
    where
        S2: BuildHasher,
    {
        Intersection::new(self, other)
    }

    /// Return an iterator over all values that are in `self` or `other`.
    ///
    /// Values from `self` are produced in their original order, followed by
    /// values that are unique to `other` in their original order.
    pub fn union<'a, S2>(&'a self, other: &'a IndexSet<T, S2>) -> Union<'a, T, S>
    where
        S2: BuildHasher,
    {
        Union::new(self, other)
    }

    /// Creates a splicing iterator that replaces the specified range in the set
    /// with the given `replace_with` iterator and yields the removed items.
    /// `replace_with` does not need to be the same length as `range`.
    ///
    /// The `range` is removed even if the iterator is not consumed until the
    /// end. It is unspecified how many elements are removed from the set if the
    /// `Splice` value is leaked.
    ///
    /// The input iterator `replace_with` is only consumed when the `Splice`
    /// value is dropped. If a value from the iterator matches an existing entry
    /// in the set (outside of `range`), then the original will be unchanged.
    /// Otherwise, the new value will be inserted in the replaced `range`.
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use indexmap::IndexSet;
    ///
    /// let mut set = IndexSet::from([0, 1, 2, 3, 4]);
    /// let new = [5, 4, 3, 2, 1];
    /// let removed: Vec<_> = set.splice(2..4, new).collect();
    ///
    /// // 1 and 4 kept their positions, while 5, 3, and 2 were newly inserted.
    /// assert!(set.into_iter().eq([0, 1, 5, 3, 2, 4]));
    /// assert_eq!(removed, &[2, 3]);
    /// ```
    #[track_caller]
    pub fn splice<R, I>(&mut self, range: R, replace_with: I) -> Splice<'_, I::IntoIter, T, S>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = T>,
    {
        Splice::new(self, range, replace_with.into_iter())
    }

    /// Moves all values from `other` into `self`, leaving `other` empty.
    ///
    /// This is equivalent to calling [`insert`][Self::insert] for each value
    /// from `other` in order, which means that values that already exist
    /// in `self` are unchanged in their current position.
    ///
    /// See also [`union`][Self::union] to iterate the combined values by
    /// reference, without modifying `self` or `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// use indexmap::IndexSet;
    ///
    /// let mut a = IndexSet::from([3, 2, 1]);
    /// let mut b = IndexSet::from([3, 4, 5]);
    /// let old_capacity = b.capacity();
    ///
    /// a.append(&mut b);
    ///
    /// assert_eq!(a.len(), 5);
    /// assert_eq!(b.len(), 0);
    /// assert_eq!(b.capacity(), old_capacity);
    ///
    /// assert!(a.iter().eq(&[3, 2, 1, 4, 5]));
    /// ```
    pub fn append<S2>(&mut self, other: &mut IndexSet<T, S2>) {
        self.map.append(&mut other.map);
    }
}

impl<T, S> IndexSet<T, S>
where
    S: BuildHasher,
{
    /// Return `true` if an equivalent to `value` exists in the set.
    ///
    /// Computes in **O(1)** time (average).
    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.contains_key(value)
    }

    /// Return a reference to the value stored in the set, if it is present,
    /// else `None`.
    ///
    /// Computes in **O(1)** time (average).
    pub fn get<Q>(&self, value: &Q) -> Option<&T>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.get_key_value(value).map(|(x, &())| x)
    }

    /// Return item index and value
    pub fn get_full<Q>(&self, value: &Q) -> Option<(usize, &T)>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.get_full(value).map(|(i, x, &())| (i, x))
    }

    /// Return item index, if it exists in the set
    ///
    /// Computes in **O(1)** time (average).
    pub fn get_index_of<Q>(&self, value: &Q) -> Option<usize>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.get_index_of(value)
    }

    /// Remove the value from the set, and return `true` if it was present.
    ///
    /// **NOTE:** This is equivalent to [`.swap_remove(value)`][Self::swap_remove], replacing this
    /// value's position with the last element, and it is deprecated in favor of calling that
    /// explicitly. If you need to preserve the relative order of the values in the set, use
    /// [`.shift_remove(value)`][Self::shift_remove] instead.
    #[deprecated(note = "`remove` disrupts the set order -- \
        use `swap_remove` or `shift_remove` for explicit behavior.")]
    pub fn remove<Q>(&mut self, value: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.swap_remove(value)
    }

    /// Remove the value from the set, and return `true` if it was present.
    ///
    /// Like [`Vec::swap_remove`], the value is removed by swapping it with the
    /// last element of the set and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `false` if `value` was not in the set.
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove<Q>(&mut self, value: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.swap_remove(value).is_some()
    }

    /// Remove the value from the set, and return `true` if it was present.
    ///
    /// Like [`Vec::remove`], the value is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `false` if `value` was not in the set.
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove<Q>(&mut self, value: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.shift_remove(value).is_some()
    }

    /// Removes and returns the value in the set, if any, that is equal to the
    /// given one.
    ///
    /// **NOTE:** This is equivalent to [`.swap_take(value)`][Self::swap_take], replacing this
    /// value's position with the last element, and it is deprecated in favor of calling that
    /// explicitly. If you need to preserve the relative order of the values in the set, use
    /// [`.shift_take(value)`][Self::shift_take] instead.
    #[deprecated(note = "`take` disrupts the set order -- \
        use `swap_take` or `shift_take` for explicit behavior.")]
    pub fn take<Q>(&mut self, value: &Q) -> Option<T>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.swap_take(value)
    }

    /// Removes and returns the value in the set, if any, that is equal to the
    /// given one.
    ///
    /// Like [`Vec::swap_remove`], the value is removed by swapping it with the
    /// last element of the set and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `value` was not in the set.
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_take<Q>(&mut self, value: &Q) -> Option<T>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.swap_remove_entry(value).map(|(x, ())| x)
    }

    /// Removes and returns the value in the set, if any, that is equal to the
    /// given one.
    ///
    /// Like [`Vec::remove`], the value is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `value` was not in the set.
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_take<Q>(&mut self, value: &Q) -> Option<T>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.shift_remove_entry(value).map(|(x, ())| x)
    }

    /// Remove the value from the set return it and the index it had.
    ///
    /// Like [`Vec::swap_remove`], the value is removed by swapping it with the
    /// last element of the set and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `value` was not in the set.
    pub fn swap_remove_full<Q>(&mut self, value: &Q) -> Option<(usize, T)>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.swap_remove_full(value).map(|(i, x, ())| (i, x))
    }

    /// Remove the value from the set return it and the index it had.
    ///
    /// Like [`Vec::remove`], the value is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `value` was not in the set.
    pub fn shift_remove_full<Q>(&mut self, value: &Q) -> Option<(usize, T)>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.map.shift_remove_full(value).map(|(i, x, ())| (i, x))
    }
}

impl<T, S> IndexSet<T, S> {
    /// Remove the last value
    ///
    /// This preserves the order of the remaining elements.
    ///
    /// Computes in **O(1)** time (average).
    #[doc(alias = "pop_last")] // like `BTreeSet`
    pub fn pop(&mut self) -> Option<T> {
        self.map.pop().map(|(x, ())| x)
    }

    /// Removes and returns the last value from a set if the predicate
    /// returns `true`, or [`None`] if the predicate returns false or the set
    /// is empty (the predicate will not be called in that case).
    ///
    /// This preserves the order of the remaining elements.
    ///
    /// Computes in **O(1)** time (average).
    ///
    /// # Examples
    ///
    /// ```
    /// use indexmap::IndexSet;
    ///
    /// let mut set = IndexSet::from([1, 2, 3, 4]);
    /// let pred = |x: &i32| *x % 2 == 0;
    ///
    /// assert_eq!(set.pop_if(pred), Some(4));
    /// assert_eq!(set.as_slice(), &[1, 2, 3]);
    /// assert_eq!(set.pop_if(pred), None);
    /// ```
    pub fn pop_if(&mut self, predicate: impl FnOnce(&T) -> bool) -> Option<T> {
        let last = self.last()?;
        if predicate(last) {
            self.pop()
        } else {
            None
        }
    }

    /// Scan through each value in the set and keep those where the
    /// closure `keep` returns `true`.
    ///
    /// The elements are visited in order, and remaining elements keep their
    /// order.
    ///
    /// Computes in **O(n)** time (average).
    pub fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.map.retain(move |x, &mut ()| keep(x))
    }

    /// Sort the set's values by their default ordering.
    ///
    /// This is a stable sort -- but equivalent values should not normally coexist in
    /// a set at all, so [`sort_unstable`][Self::sort_unstable] is preferred
    /// because it is generally faster and doesn't allocate auxiliary memory.
    ///
    /// See [`sort_by`](Self::sort_by) for details.
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.map.sort_keys()
    }

    /// Sort the set's values in place using the comparison function `cmp`.
    ///
    /// Computes in **O(n log n)** time and **O(n)** space. The sort is stable.
    pub fn sort_by<F>(&mut self, mut cmp: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.map.sort_by(move |a, (), b, ()| cmp(a, b));
    }

    /// Sort the values of the set and return a by-value iterator of
    /// the values with the result.
    ///
    /// The sort is stable.
    pub fn sorted_by<F>(self, mut cmp: F) -> IntoIter<T>
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        let mut entries = self.into_entries();
        entries.sort_by(move |a, b| cmp(&a.key, &b.key));
        IntoIter::new(entries)
    }

    /// Sort the set's values in place using a key extraction function.
    ///
    /// Computes in **O(n log n)** time and **O(n)** space. The sort is stable.
    pub fn sort_by_key<K, F>(&mut self, mut sort_key: F)
    where
        K: Ord,
        F: FnMut(&T) -> K,
    {
        self.with_entries(move |entries| {
            entries.sort_by_key(move |a| sort_key(&a.key));
        });
    }

    /// Sort the set's values by their default ordering.
    ///
    /// See [`sort_unstable_by`](Self::sort_unstable_by) for details.
    pub fn sort_unstable(&mut self)
    where
        T: Ord,
    {
        self.map.sort_unstable_keys()
    }

    /// Sort the set's values in place using the comparison function `cmp`.
    ///
    /// Computes in **O(n log n)** time. The sort is unstable.
    pub fn sort_unstable_by<F>(&mut self, mut cmp: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.map.sort_unstable_by(move |a, _, b, _| cmp(a, b))
    }

    /// Sort the values of the set and return a by-value iterator of
    /// the values with the result.
    pub fn sorted_unstable_by<F>(self, mut cmp: F) -> IntoIter<T>
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        let mut entries = self.into_entries();
        entries.sort_unstable_by(move |a, b| cmp(&a.key, &b.key));
        IntoIter::new(entries)
    }

    /// Sort the set's values in place using a key extraction function.
    ///
    /// Computes in **O(n log n)** time. The sort is unstable.
    pub fn sort_unstable_by_key<K, F>(&mut self, mut sort_key: F)
    where
        K: Ord,
        F: FnMut(&T) -> K,
    {
        self.with_entries(move |entries| {
            entries.sort_unstable_by_key(move |a| sort_key(&a.key));
        });
    }

    /// Sort the set's values in place using a key extraction function.
    ///
    /// During sorting, the function is called at most once per entry, by using temporary storage
    /// to remember the results of its evaluation. The order of calls to the function is
    /// unspecified and may change between versions of `indexmap` or the standard library.
    ///
    /// Computes in **O(m n + n log n + c)** time () and **O(n)** space, where the function is
    /// **O(m)**, *n* is the length of the map, and *c* the capacity. The sort is stable.
    pub fn sort_by_cached_key<K, F>(&mut self, mut sort_key: F)
    where
        K: Ord,
        F: FnMut(&T) -> K,
    {
        self.with_entries(move |entries| {
            entries.sort_by_cached_key(move |a| sort_key(&a.key));
        });
    }

    /// Search over a sorted set for a value.
    ///
    /// Returns the position where that value is present, or the position where it can be inserted
    /// to maintain the sort. See [`slice::binary_search`] for more details.
    ///
    /// Computes in **O(log(n))** time, which is notably less scalable than looking the value up
    /// using [`get_index_of`][IndexSet::get_index_of], but this can also position missing values.
    pub fn binary_search(&self, x: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        self.as_slice().binary_search(x)
    }

    /// Search over a sorted set with a comparator function.
    ///
    /// Returns the position where that value is present, or the position where it can be inserted
    /// to maintain the sort. See [`slice::binary_search_by`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[inline]
    pub fn binary_search_by<'a, F>(&'a self, f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> Ordering,
    {
        self.as_slice().binary_search_by(f)
    }

    /// Search over a sorted set with an extraction function.
    ///
    /// Returns the position where that value is present, or the position where it can be inserted
    /// to maintain the sort. See [`slice::binary_search_by_key`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[inline]
    pub fn binary_search_by_key<'a, B, F>(&'a self, b: &B, f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> B,
        B: Ord,
    {
        self.as_slice().binary_search_by_key(b, f)
    }

    /// Checks if the values of this set are sorted.
    #[inline]
    pub fn is_sorted(&self) -> bool
    where
        T: PartialOrd,
    {
        self.as_slice().is_sorted()
    }

    /// Checks if this set is sorted using the given comparator function.
    #[inline]
    pub fn is_sorted_by<'a, F>(&'a self, cmp: F) -> bool
    where
        F: FnMut(&'a T, &'a T) -> bool,
    {
        self.as_slice().is_sorted_by(cmp)
    }

    /// Checks if this set is sorted using the given sort-key function.
    #[inline]
    pub fn is_sorted_by_key<'a, F, K>(&'a self, sort_key: F) -> bool
    where
        F: FnMut(&'a T) -> K,
        K: PartialOrd,
    {
        self.as_slice().is_sorted_by_key(sort_key)
    }

    /// Returns the index of the partition point of a sorted set according to the given predicate
    /// (the index of the first element of the second partition).
    ///
    /// See [`slice::partition_point`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[must_use]
    pub fn partition_point<P>(&self, pred: P) -> usize
    where
        P: FnMut(&T) -> bool,
    {
        self.as_slice().partition_point(pred)
    }

    /// Reverses the order of the set's values in place.
    ///
    /// Computes in **O(n)** time and **O(1)** space.
    pub fn reverse(&mut self) {
        self.map.reverse()
    }

    /// Returns a slice of all the values in the set.
    ///
    /// Computes in **O(1)** time.
    pub fn as_slice(&self) -> &Slice<T> {
        Slice::from_slice(self.as_entries())
    }

    /// Converts into a boxed slice of all the values in the set.
    ///
    /// Note that this will drop the inner hash table and any excess capacity.
    pub fn into_boxed_slice(self) -> Box<Slice<T>> {
        Slice::from_boxed(self.into_entries().into_boxed_slice())
    }

    /// Get a value by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    pub fn get_index(&self, index: usize) -> Option<&T> {
        self.as_entries().get(index).map(Bucket::key_ref)
    }

    /// Returns a slice of values in the given range of indices.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    pub fn get_range<R: RangeBounds<usize>>(&self, range: R) -> Option<&Slice<T>> {
        let entries = self.as_entries();
        let range = try_simplify_range(range, entries.len())?;
        entries.get(range).map(Slice::from_slice)
    }

    /// Get the first value
    ///
    /// Computes in **O(1)** time.
    pub fn first(&self) -> Option<&T> {
        self.as_entries().first().map(Bucket::key_ref)
    }

    /// Get the last value
    ///
    /// Computes in **O(1)** time.
    pub fn last(&self) -> Option<&T> {
        self.as_entries().last().map(Bucket::key_ref)
    }

    /// Remove the value by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Like [`Vec::swap_remove`], the value is removed by swapping it with the
    /// last element of the set and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove_index(&mut self, index: usize) -> Option<T> {
        self.map.swap_remove_index(index).map(|(x, ())| x)
    }

    /// Remove the value by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Like [`Vec::remove`], the value is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove_index(&mut self, index: usize) -> Option<T> {
        self.map.shift_remove_index(index).map(|(x, ())| x)
    }

    /// Moves the position of a value from one index to another
    /// by shifting all other values in-between.
    ///
    /// * If `from < to`, the other values will shift down while the targeted value moves up.
    /// * If `from > to`, the other values will shift up while the targeted value moves down.
    ///
    /// ***Panics*** if `from` or `to` are out of bounds.
    ///
    /// Computes in **O(n)** time (average).
    #[track_caller]
    pub fn move_index(&mut self, from: usize, to: usize) {
        self.map.move_index(from, to)
    }

    /// Swaps the position of two values in the set.
    ///
    /// ***Panics*** if `a` or `b` are out of bounds.
    ///
    /// Computes in **O(1)** time (average).
    #[track_caller]
    pub fn swap_indices(&mut self, a: usize, b: usize) {
        self.map.swap_indices(a, b)
    }
}

/// Access [`IndexSet`] values at indexed positions.
///
/// # Examples
///
/// ```
/// use indexmap::IndexSet;
///
/// let mut set = IndexSet::new();
/// for word in "Lorem ipsum dolor sit amet".split_whitespace() {
///     set.insert(word.to_string());
/// }
/// assert_eq!(set[0], "Lorem");
/// assert_eq!(set[1], "ipsum");
/// set.reverse();
/// assert_eq!(set[0], "amet");
/// assert_eq!(set[1], "sit");
/// set.sort();
/// assert_eq!(set[0], "Lorem");
/// assert_eq!(set[1], "amet");
/// ```
///
/// ```should_panic
/// use indexmap::IndexSet;
///
/// let mut set = IndexSet::new();
/// set.insert("foo");
/// println!("{:?}", set[10]); // panics!
/// ```
impl<T, S> Index<usize> for IndexSet<T, S> {
    type Output = T;

    /// Returns a reference to the value at the supplied `index`.
    ///
    /// ***Panics*** if `index` is out of bounds.
    fn index(&self, index: usize) -> &T {
        if let Some(value) = self.get_index(index) {
            value
        } else {
            panic!(
                "index out of bounds: the len is {len} but the index is {index}",
                len = self.len()
            );
        }
    }
}

impl<T, S> FromIterator<T> for IndexSet<T, S>
where
    T: Hash + Eq,
    S: BuildHasher + Default,
{
    fn from_iter<I: IntoIterator<Item = T>>(iterable: I) -> Self {
        let iter = iterable.into_iter().map(|x| (x, ()));
        IndexSet {
            map: IndexMap::from_iter(iter),
        }
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<T, const N: usize> From<[T; N]> for IndexSet<T, RandomState>
where
    T: Eq + Hash,
{
    /// # Examples
    ///
    /// ```
    /// use indexmap::IndexSet;
    ///
    /// let set1 = IndexSet::from([1, 2, 3, 4]);
    /// let set2: IndexSet<_> = [1, 2, 3, 4].into();
    /// assert_eq!(set1, set2);
    /// ```
    fn from(arr: [T; N]) -> Self {
        Self::from_iter(arr)
    }
}

impl<T, S> Extend<T> for IndexSet<T, S>
where
    T: Hash + Eq,
    S: BuildHasher,
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iterable: I) {
        let iter = iterable.into_iter().map(|x| (x, ()));
        self.map.extend(iter);
    }
}

impl<'a, T, S> Extend<&'a T> for IndexSet<T, S>
where
    T: Hash + Eq + Copy + 'a,
    S: BuildHasher,
{
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iterable: I) {
        let iter = iterable.into_iter().copied();
        self.extend(iter);
    }
}

impl<T, S> Default for IndexSet<T, S>
where
    S: Default,
{
    /// Return an empty [`IndexSet`]
    fn default() -> Self {
        IndexSet {
            map: IndexMap::default(),
        }
    }
}

impl<T, S1, S2> PartialEq<IndexSet<T, S2>> for IndexSet<T, S1>
where
    T: Hash + Eq,
    S1: BuildHasher,
    S2: BuildHasher,
{
    fn eq(&self, other: &IndexSet<T, S2>) -> bool {
        self.len() == other.len() && self.is_subset(other)
    }
}

impl<T, S> Eq for IndexSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
}

impl<T, S> IndexSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    /// Returns `true` if `self` has no elements in common with `other`.
    pub fn is_disjoint<S2>(&self, other: &IndexSet<T, S2>) -> bool
    where
        S2: BuildHasher,
    {
        if self.len() <= other.len() {
            self.iter().all(move |value| !other.contains(value))
        } else {
            other.iter().all(move |value| !self.contains(value))
        }
    }

    /// Returns `true` if all elements of `self` are contained in `other`.
    pub fn is_subset<S2>(&self, other: &IndexSet<T, S2>) -> bool
    where
        S2: BuildHasher,
    {
        self.len() <= other.len() && self.iter().all(move |value| other.contains(value))
    }

    /// Returns `true` if all elements of `other` are contained in `self`.
    pub fn is_superset<S2>(&self, other: &IndexSet<T, S2>) -> bool
    where
        S2: BuildHasher,
    {
        other.is_subset(self)
    }
}

impl<T, S1, S2> BitAnd<&IndexSet<T, S2>> for &IndexSet<T, S1>
where
    T: Eq + Hash + Clone,
    S1: BuildHasher + Default,
    S2: BuildHasher,
{
    type Output = IndexSet<T, S1>;

    /// Returns the set intersection, cloned into a new set.
    ///
    /// Values are collected in the same order that they appear in `self`.
    fn bitand(self, other: &IndexSet<T, S2>) -> Self::Output {
        self.intersection(other).cloned().collect()
    }
}

impl<T, S1, S2> BitOr<&IndexSet<T, S2>> for &IndexSet<T, S1>
where
    T: Eq + Hash + Clone,
    S1: BuildHasher + Default,
    S2: BuildHasher,
{
    type Output = IndexSet<T, S1>;

    /// Returns the set union, cloned into a new set.
    ///
    /// Values from `self` are collected in their original order, followed by
    /// values that are unique to `other` in their original order.
    fn bitor(self, other: &IndexSet<T, S2>) -> Self::Output {
        self.union(other).cloned().collect()
    }
}

impl<T, S1, S2> BitXor<&IndexSet<T, S2>> for &IndexSet<T, S1>
where
    T: Eq + Hash + Clone,
    S1: BuildHasher + Default,
    S2: BuildHasher,
{
    type Output = IndexSet<T, S1>;

    /// Returns the set symmetric-difference, cloned into a new set.
    ///
    /// Values from `self` are collected in their original order, followed by
    /// values from `other` in their original order.
    fn bitxor(self, other: &IndexSet<T, S2>) -> Self::Output {
        self.symmetric_difference(other).cloned().collect()
    }
}

impl<T, S1, S2> Sub<&IndexSet<T, S2>> for &IndexSet<T, S1>
where
    T: Eq + Hash + Clone,
    S1: BuildHasher + Default,
    S2: BuildHasher,
{
    type Output = IndexSet<T, S1>;

    /// Returns the set difference, cloned into a new set.
    ///
    /// Values are collected in the same order that they appear in `self`.
    fn sub(self, other: &IndexSet<T, S2>) -> Self::Output {
        self.difference(other).cloned().collect()
    }
}
