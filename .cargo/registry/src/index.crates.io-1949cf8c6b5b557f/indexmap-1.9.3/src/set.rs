//! A hash set implemented using `IndexMap`

#[cfg(feature = "rayon")]
pub use crate::rayon::set as rayon;

#[cfg(has_std)]
use std::collections::hash_map::RandomState;

use crate::vec::{self, Vec};
use core::cmp::Ordering;
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::iter::{Chain, FusedIterator};
use core::ops::{BitAnd, BitOr, BitXor, Index, RangeBounds, Sub};
use core::slice;

use super::{Entries, Equivalent, IndexMap};

type Bucket<T> = super::Bucket<T, ()>;

/// A hash set where the iteration order of the values is independent of their
/// hash values.
///
/// The interface is closely compatible with the standard `HashSet`, but also
/// has additional features.
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
/// `union` produce a concatenated order, as do their matching "bitwise"
/// operators.  See their documentation for specifics.
///
/// The insertion order is preserved, with **notable exceptions** like the
/// `.remove()` or `.swap_remove()` methods. Methods such as `.sort_by()` of
/// course result in a new order, depending on the sorting order.
///
/// # Indices
///
/// The values are indexed in a compact range without holes in the range
/// `0..self.len()`. For example, the method `.get_full` looks up the index for
/// a value, and the method `.get_index` looks up the value by index.
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
#[cfg(has_std)]
pub struct IndexSet<T, S = RandomState> {
    pub(crate) map: IndexMap<T, (), S>,
}
#[cfg(not(has_std))]
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

impl<T, S> Entries for IndexSet<T, S> {
    type Entry = Bucket<T>;

    #[inline]
    fn into_entries(self) -> Vec<Self::Entry> {
        self.map.into_entries()
    }

    #[inline]
    fn as_entries(&self) -> &[Self::Entry] {
        self.map.as_entries()
    }

    #[inline]
    fn as_entries_mut(&mut self) -> &mut [Self::Entry] {
        self.map.as_entries_mut()
    }

    fn with_entries<F>(&mut self, f: F)
    where
        F: FnOnce(&mut [Self::Entry]),
    {
        self.map.with_entries(f);
    }
}

impl<T, S> fmt::Debug for IndexSet<T, S>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(not(feature = "test_debug")) {
            f.debug_set().entries(self.iter()).finish()
        } else {
            // Let the inner `IndexMap` print all of its details
            f.debug_struct("IndexSet").field("map", &self.map).finish()
        }
    }
}

#[cfg(has_std)]
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
        Iter {
            iter: self.map.as_entries().iter(),
        }
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
    /// The range may be any type that implements `RangeBounds<usize>`,
    /// including all of the `std::ops::Range*` types, or even a tuple pair of
    /// `Bound` start and end values. To drain the set entirely, use `RangeFull`
    /// like `set.drain(..)`.
    ///
    /// This shifts down all entries following the drained range to fill the
    /// gap, and keeps the allocated memory for reuse.
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the set.
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, T>
    where
        R: RangeBounds<usize>,
    {
        Drain {
            iter: self.map.drain(range).iter,
        }
    }

    /// Splits the collection into two at the given index.
    ///
    /// Returns a newly allocated set containing the elements in the range
    /// `[at, len)`. After the call, the original set will be left containing
    /// the elements `[0, at)` with its previous capacity unchanged.
    ///
    /// ***Panics*** if `at > len`.
    pub fn split_off(&mut self, at: usize) -> Self
    where
        S: Clone,
    {
        Self {
            map: self.map.split_off(at),
        }
    }
}

impl<T, S> IndexSet<T, S>
where
    T: Hash + Eq,
    S: BuildHasher,
{
    /// Reserve capacity for `additional` more values.
    ///
    /// Computes in **O(n)** time.
    pub fn reserve(&mut self, additional: usize) {
        self.map.reserve(additional);
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
        use super::map::Entry::*;

        match self.map.entry(value) {
            Occupied(e) => (e.index(), false),
            Vacant(e) => {
                let index = e.index();
                e.insert(());
                (index, true)
            }
        }
    }

    /// Return an iterator over the values that are in `self` but not `other`.
    ///
    /// Values are produced in the same order that they appear in `self`.
    pub fn difference<'a, S2>(&'a self, other: &'a IndexSet<T, S2>) -> Difference<'a, T, S2>
    where
        S2: BuildHasher,
    {
        Difference {
            iter: self.iter(),
            other,
        }
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
        SymmetricDifference {
            iter: self.difference(other).chain(other.difference(self)),
        }
    }

    /// Return an iterator over the values that are in both `self` and `other`.
    ///
    /// Values are produced in the same order that they appear in `self`.
    pub fn intersection<'a, S2>(&'a self, other: &'a IndexSet<T, S2>) -> Intersection<'a, T, S2>
    where
        S2: BuildHasher,
    {
        Intersection {
            iter: self.iter(),
            other,
        }
    }

    /// Return an iterator over all values that are in `self` or `other`.
    ///
    /// Values from `self` are produced in their original order, followed by
    /// values that are unique to `other` in their original order.
    pub fn union<'a, S2>(&'a self, other: &'a IndexSet<T, S2>) -> Union<'a, T, S>
    where
        S2: BuildHasher,
    {
        Union {
            iter: self.iter().chain(other.difference(self)),
        }
    }

    /// Return `true` if an equivalent to `value` exists in the set.
    ///
    /// Computes in **O(1)** time (average).
    pub fn contains<Q: ?Sized>(&self, value: &Q) -> bool
    where
        Q: Hash + Equivalent<T>,
    {
        self.map.contains_key(value)
    }

    /// Return a reference to the value stored in the set, if it is present,
    /// else `None`.
    ///
    /// Computes in **O(1)** time (average).
    pub fn get<Q: ?Sized>(&self, value: &Q) -> Option<&T>
    where
        Q: Hash + Equivalent<T>,
    {
        self.map.get_key_value(value).map(|(x, &())| x)
    }

    /// Return item index and value
    pub fn get_full<Q: ?Sized>(&self, value: &Q) -> Option<(usize, &T)>
    where
        Q: Hash + Equivalent<T>,
    {
        self.map.get_full(value).map(|(i, x, &())| (i, x))
    }

    /// Return item index, if it exists in the set
    pub fn get_index_of<Q: ?Sized>(&self, value: &Q) -> Option<usize>
    where
        Q: Hash + Equivalent<T>,
    {
        self.map.get_index_of(value)
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
        use super::map::Entry::*;

        match self.map.entry(value) {
            Vacant(e) => {
                let index = e.index();
                e.insert(());
                (index, None)
            }
            Occupied(e) => (e.index(), Some(e.replace_key())),
        }
    }

    /// Remove the value from the set, and return `true` if it was present.
    ///
    /// **NOTE:** This is equivalent to `.swap_remove(value)`, if you want
    /// to preserve the order of the values in the set, use `.shift_remove(value)`.
    ///
    /// Computes in **O(1)** time (average).
    pub fn remove<Q: ?Sized>(&mut self, value: &Q) -> bool
    where
        Q: Hash + Equivalent<T>,
    {
        self.swap_remove(value)
    }

    /// Remove the value from the set, and return `true` if it was present.
    ///
    /// Like `Vec::swap_remove`, the value is removed by swapping it with the
    /// last element of the set and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `false` if `value` was not in the set.
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove<Q: ?Sized>(&mut self, value: &Q) -> bool
    where
        Q: Hash + Equivalent<T>,
    {
        self.map.swap_remove(value).is_some()
    }

    /// Remove the value from the set, and return `true` if it was present.
    ///
    /// Like `Vec::remove`, the value is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `false` if `value` was not in the set.
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove<Q: ?Sized>(&mut self, value: &Q) -> bool
    where
        Q: Hash + Equivalent<T>,
    {
        self.map.shift_remove(value).is_some()
    }

    /// Removes and returns the value in the set, if any, that is equal to the
    /// given one.
    ///
    /// **NOTE:** This is equivalent to `.swap_take(value)`, if you need to
    /// preserve the order of the values in the set, use `.shift_take(value)`
    /// instead.
    ///
    /// Computes in **O(1)** time (average).
    pub fn take<Q: ?Sized>(&mut self, value: &Q) -> Option<T>
    where
        Q: Hash + Equivalent<T>,
    {
        self.swap_take(value)
    }

    /// Removes and returns the value in the set, if any, that is equal to the
    /// given one.
    ///
    /// Like `Vec::swap_remove`, the value is removed by swapping it with the
    /// last element of the set and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `value` was not in the set.
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_take<Q: ?Sized>(&mut self, value: &Q) -> Option<T>
    where
        Q: Hash + Equivalent<T>,
    {
        self.map.swap_remove_entry(value).map(|(x, ())| x)
    }

    /// Removes and returns the value in the set, if any, that is equal to the
    /// given one.
    ///
    /// Like `Vec::remove`, the value is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `value` was not in the set.
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_take<Q: ?Sized>(&mut self, value: &Q) -> Option<T>
    where
        Q: Hash + Equivalent<T>,
    {
        self.map.shift_remove_entry(value).map(|(x, ())| x)
    }

    /// Remove the value from the set return it and the index it had.
    ///
    /// Like `Vec::swap_remove`, the value is removed by swapping it with the
    /// last element of the set and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `value` was not in the set.
    pub fn swap_remove_full<Q: ?Sized>(&mut self, value: &Q) -> Option<(usize, T)>
    where
        Q: Hash + Equivalent<T>,
    {
        self.map.swap_remove_full(value).map(|(i, x, ())| (i, x))
    }

    /// Remove the value from the set return it and the index it had.
    ///
    /// Like `Vec::remove`, the value is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `value` was not in the set.
    pub fn shift_remove_full<Q: ?Sized>(&mut self, value: &Q) -> Option<(usize, T)>
    where
        Q: Hash + Equivalent<T>,
    {
        self.map.shift_remove_full(value).map(|(i, x, ())| (i, x))
    }

    /// Remove the last value
    ///
    /// This preserves the order of the remaining elements.
    ///
    /// Computes in **O(1)** time (average).
    pub fn pop(&mut self) -> Option<T> {
        self.map.pop().map(|(x, ())| x)
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

    /// Sort the set’s values by their default ordering.
    ///
    /// See [`sort_by`](Self::sort_by) for details.
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.map.sort_keys()
    }

    /// Sort the set’s values in place using the comparison function `cmp`.
    ///
    /// Computes in **O(n log n)** time and **O(n)** space. The sort is stable.
    pub fn sort_by<F>(&mut self, mut cmp: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.map.sort_by(move |a, _, b, _| cmp(a, b));
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
        IntoIter {
            iter: entries.into_iter(),
        }
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

    /// Sort the set's values in place using the comparison funtion `cmp`.
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
        IntoIter {
            iter: entries.into_iter(),
        }
    }

    /// Reverses the order of the set’s values in place.
    ///
    /// Computes in **O(n)** time and **O(1)** space.
    pub fn reverse(&mut self) {
        self.map.reverse()
    }
}

impl<T, S> IndexSet<T, S> {
    /// Get a value by index
    ///
    /// Valid indices are *0 <= index < self.len()*
    ///
    /// Computes in **O(1)** time.
    pub fn get_index(&self, index: usize) -> Option<&T> {
        self.as_entries().get(index).map(Bucket::key_ref)
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
    /// Valid indices are *0 <= index < self.len()*
    ///
    /// Like `Vec::swap_remove`, the value is removed by swapping it with the
    /// last element of the set and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove_index(&mut self, index: usize) -> Option<T> {
        self.map.swap_remove_index(index).map(|(x, ())| x)
    }

    /// Remove the value by index
    ///
    /// Valid indices are *0 <= index < self.len()*
    ///
    /// Like `Vec::remove`, the value is removed by shifting all of the
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
    pub fn move_index(&mut self, from: usize, to: usize) {
        self.map.move_index(from, to)
    }

    /// Swaps the position of two values in the set.
    ///
    /// ***Panics*** if `a` or `b` are out of bounds.
    pub fn swap_indices(&mut self, a: usize, b: usize) {
        self.map.swap_indices(a, b)
    }
}

/// Access `IndexSet` values at indexed positions.
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
        self.get_index(index)
            .expect("IndexSet: index out of bounds")
    }
}

/// An owning iterator over the items of a `IndexSet`.
///
/// This `struct` is created by the [`into_iter`] method on [`IndexSet`]
/// (provided by the `IntoIterator` trait). See its documentation for more.
///
/// [`IndexSet`]: struct.IndexSet.html
/// [`into_iter`]: struct.IndexSet.html#method.into_iter
pub struct IntoIter<T> {
    iter: vec::IntoIter<Bucket<T>>,
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

/// An iterator over the items of a `IndexSet`.
///
/// This `struct` is created by the [`iter`] method on [`IndexSet`].
/// See its documentation for more.
///
/// [`IndexSet`]: struct.IndexSet.html
/// [`iter`]: struct.IndexSet.html#method.iter
pub struct Iter<'a, T> {
    iter: slice::Iter<'a, Bucket<T>>,
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

/// A draining iterator over the items of a `IndexSet`.
///
/// This `struct` is created by the [`drain`] method on [`IndexSet`].
/// See its documentation for more.
///
/// [`IndexSet`]: struct.IndexSet.html
/// [`drain`]: struct.IndexSet.html#method.drain
pub struct Drain<'a, T> {
    iter: vec::Drain<'a, Bucket<T>>,
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
        IntoIter {
            iter: self.into_entries().into_iter(),
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

#[cfg(has_std)]
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
    /// Return an empty `IndexSet`
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

/// A lazy iterator producing elements in the difference of `IndexSet`s.
///
/// This `struct` is created by the [`difference`] method on [`IndexSet`].
/// See its documentation for more.
///
/// [`IndexSet`]: struct.IndexSet.html
/// [`difference`]: struct.IndexSet.html#method.difference
pub struct Difference<'a, T, S> {
    iter: Iter<'a, T>,
    other: &'a IndexSet<T, S>,
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

/// A lazy iterator producing elements in the intersection of `IndexSet`s.
///
/// This `struct` is created by the [`intersection`] method on [`IndexSet`].
/// See its documentation for more.
///
/// [`IndexSet`]: struct.IndexSet.html
/// [`intersection`]: struct.IndexSet.html#method.intersection
pub struct Intersection<'a, T, S> {
    iter: Iter<'a, T>,
    other: &'a IndexSet<T, S>,
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

/// A lazy iterator producing elements in the symmetric difference of `IndexSet`s.
///
/// This `struct` is created by the [`symmetric_difference`] method on
/// [`IndexSet`]. See its documentation for more.
///
/// [`IndexSet`]: struct.IndexSet.html
/// [`symmetric_difference`]: struct.IndexSet.html#method.symmetric_difference
pub struct SymmetricDifference<'a, T, S1, S2> {
    iter: Chain<Difference<'a, T, S2>, Difference<'a, T, S1>>,
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

/// A lazy iterator producing elements in the union of `IndexSet`s.
///
/// This `struct` is created by the [`union`] method on [`IndexSet`].
/// See its documentation for more.
///
/// [`IndexSet`]: struct.IndexSet.html
/// [`union`]: struct.IndexSet.html#method.union
pub struct Union<'a, T, S> {
    iter: Chain<Iter<'a, T>, Difference<'a, T, S>>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::String;

    #[test]
    fn it_works() {
        let mut set = IndexSet::new();
        assert_eq!(set.is_empty(), true);
        set.insert(1);
        set.insert(1);
        assert_eq!(set.len(), 1);
        assert!(set.get(&1).is_some());
        assert_eq!(set.is_empty(), false);
    }

    #[test]
    fn new() {
        let set = IndexSet::<String>::new();
        println!("{:?}", set);
        assert_eq!(set.capacity(), 0);
        assert_eq!(set.len(), 0);
        assert_eq!(set.is_empty(), true);
    }

    #[test]
    fn insert() {
        let insert = [0, 4, 2, 12, 8, 7, 11, 5];
        let not_present = [1, 3, 6, 9, 10];
        let mut set = IndexSet::with_capacity(insert.len());

        for (i, &elt) in insert.iter().enumerate() {
            assert_eq!(set.len(), i);
            set.insert(elt);
            assert_eq!(set.len(), i + 1);
            assert_eq!(set.get(&elt), Some(&elt));
        }
        println!("{:?}", set);

        for &elt in &not_present {
            assert!(set.get(&elt).is_none());
        }
    }

    #[test]
    fn insert_full() {
        let insert = vec![9, 2, 7, 1, 4, 6, 13];
        let present = vec![1, 6, 2];
        let mut set = IndexSet::with_capacity(insert.len());

        for (i, &elt) in insert.iter().enumerate() {
            assert_eq!(set.len(), i);
            let (index, success) = set.insert_full(elt);
            assert!(success);
            assert_eq!(Some(index), set.get_full(&elt).map(|x| x.0));
            assert_eq!(set.len(), i + 1);
        }

        let len = set.len();
        for &elt in &present {
            let (index, success) = set.insert_full(elt);
            assert!(!success);
            assert_eq!(Some(index), set.get_full(&elt).map(|x| x.0));
            assert_eq!(set.len(), len);
        }
    }

    #[test]
    fn insert_2() {
        let mut set = IndexSet::with_capacity(16);

        let mut values = vec![];
        values.extend(0..16);
        values.extend(if cfg!(miri) { 32..64 } else { 128..267 });

        for &i in &values {
            let old_set = set.clone();
            set.insert(i);
            for value in old_set.iter() {
                if set.get(value).is_none() {
                    println!("old_set: {:?}", old_set);
                    println!("set: {:?}", set);
                    panic!("did not find {} in set", value);
                }
            }
        }

        for &i in &values {
            assert!(set.get(&i).is_some(), "did not find {}", i);
        }
    }

    #[test]
    fn insert_dup() {
        let mut elements = vec![0, 2, 4, 6, 8];
        let mut set: IndexSet<u8> = elements.drain(..).collect();
        {
            let (i, v) = set.get_full(&0).unwrap();
            assert_eq!(set.len(), 5);
            assert_eq!(i, 0);
            assert_eq!(*v, 0);
        }
        {
            let inserted = set.insert(0);
            let (i, v) = set.get_full(&0).unwrap();
            assert_eq!(set.len(), 5);
            assert_eq!(inserted, false);
            assert_eq!(i, 0);
            assert_eq!(*v, 0);
        }
    }

    #[test]
    fn insert_order() {
        let insert = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
        let mut set = IndexSet::new();

        for &elt in &insert {
            set.insert(elt);
        }

        assert_eq!(set.iter().count(), set.len());
        assert_eq!(set.iter().count(), insert.len());
        for (a, b) in insert.iter().zip(set.iter()) {
            assert_eq!(a, b);
        }
        for (i, v) in (0..insert.len()).zip(set.iter()) {
            assert_eq!(set.get_index(i).unwrap(), v);
        }
    }

    #[test]
    fn replace() {
        let replace = [0, 4, 2, 12, 8, 7, 11, 5];
        let not_present = [1, 3, 6, 9, 10];
        let mut set = IndexSet::with_capacity(replace.len());

        for (i, &elt) in replace.iter().enumerate() {
            assert_eq!(set.len(), i);
            set.replace(elt);
            assert_eq!(set.len(), i + 1);
            assert_eq!(set.get(&elt), Some(&elt));
        }
        println!("{:?}", set);

        for &elt in &not_present {
            assert!(set.get(&elt).is_none());
        }
    }

    #[test]
    fn replace_full() {
        let replace = vec![9, 2, 7, 1, 4, 6, 13];
        let present = vec![1, 6, 2];
        let mut set = IndexSet::with_capacity(replace.len());

        for (i, &elt) in replace.iter().enumerate() {
            assert_eq!(set.len(), i);
            let (index, replaced) = set.replace_full(elt);
            assert!(replaced.is_none());
            assert_eq!(Some(index), set.get_full(&elt).map(|x| x.0));
            assert_eq!(set.len(), i + 1);
        }

        let len = set.len();
        for &elt in &present {
            let (index, replaced) = set.replace_full(elt);
            assert_eq!(Some(elt), replaced);
            assert_eq!(Some(index), set.get_full(&elt).map(|x| x.0));
            assert_eq!(set.len(), len);
        }
    }

    #[test]
    fn replace_2() {
        let mut set = IndexSet::with_capacity(16);

        let mut values = vec![];
        values.extend(0..16);
        values.extend(if cfg!(miri) { 32..64 } else { 128..267 });

        for &i in &values {
            let old_set = set.clone();
            set.replace(i);
            for value in old_set.iter() {
                if set.get(value).is_none() {
                    println!("old_set: {:?}", old_set);
                    println!("set: {:?}", set);
                    panic!("did not find {} in set", value);
                }
            }
        }

        for &i in &values {
            assert!(set.get(&i).is_some(), "did not find {}", i);
        }
    }

    #[test]
    fn replace_dup() {
        let mut elements = vec![0, 2, 4, 6, 8];
        let mut set: IndexSet<u8> = elements.drain(..).collect();
        {
            let (i, v) = set.get_full(&0).unwrap();
            assert_eq!(set.len(), 5);
            assert_eq!(i, 0);
            assert_eq!(*v, 0);
        }
        {
            let replaced = set.replace(0);
            let (i, v) = set.get_full(&0).unwrap();
            assert_eq!(set.len(), 5);
            assert_eq!(replaced, Some(0));
            assert_eq!(i, 0);
            assert_eq!(*v, 0);
        }
    }

    #[test]
    fn replace_order() {
        let replace = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
        let mut set = IndexSet::new();

        for &elt in &replace {
            set.replace(elt);
        }

        assert_eq!(set.iter().count(), set.len());
        assert_eq!(set.iter().count(), replace.len());
        for (a, b) in replace.iter().zip(set.iter()) {
            assert_eq!(a, b);
        }
        for (i, v) in (0..replace.len()).zip(set.iter()) {
            assert_eq!(set.get_index(i).unwrap(), v);
        }
    }

    #[test]
    fn grow() {
        let insert = [0, 4, 2, 12, 8, 7, 11];
        let not_present = [1, 3, 6, 9, 10];
        let mut set = IndexSet::with_capacity(insert.len());

        for (i, &elt) in insert.iter().enumerate() {
            assert_eq!(set.len(), i);
            set.insert(elt);
            assert_eq!(set.len(), i + 1);
            assert_eq!(set.get(&elt), Some(&elt));
        }

        println!("{:?}", set);
        for &elt in &insert {
            set.insert(elt * 10);
        }
        for &elt in &insert {
            set.insert(elt * 100);
        }
        for (i, &elt) in insert.iter().cycle().enumerate().take(100) {
            set.insert(elt * 100 + i as i32);
        }
        println!("{:?}", set);
        for &elt in &not_present {
            assert!(set.get(&elt).is_none());
        }
    }

    #[test]
    fn reserve() {
        let mut set = IndexSet::<usize>::new();
        assert_eq!(set.capacity(), 0);
        set.reserve(100);
        let capacity = set.capacity();
        assert!(capacity >= 100);
        for i in 0..capacity {
            assert_eq!(set.len(), i);
            set.insert(i);
            assert_eq!(set.len(), i + 1);
            assert_eq!(set.capacity(), capacity);
            assert_eq!(set.get(&i), Some(&i));
        }
        set.insert(capacity);
        assert_eq!(set.len(), capacity + 1);
        assert!(set.capacity() > capacity);
        assert_eq!(set.get(&capacity), Some(&capacity));
    }

    #[test]
    fn shrink_to_fit() {
        let mut set = IndexSet::<usize>::new();
        assert_eq!(set.capacity(), 0);
        for i in 0..100 {
            assert_eq!(set.len(), i);
            set.insert(i);
            assert_eq!(set.len(), i + 1);
            assert!(set.capacity() >= i + 1);
            assert_eq!(set.get(&i), Some(&i));
            set.shrink_to_fit();
            assert_eq!(set.len(), i + 1);
            assert_eq!(set.capacity(), i + 1);
            assert_eq!(set.get(&i), Some(&i));
        }
    }

    #[test]
    fn remove() {
        let insert = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
        let mut set = IndexSet::new();

        for &elt in &insert {
            set.insert(elt);
        }

        assert_eq!(set.iter().count(), set.len());
        assert_eq!(set.iter().count(), insert.len());
        for (a, b) in insert.iter().zip(set.iter()) {
            assert_eq!(a, b);
        }

        let remove_fail = [99, 77];
        let remove = [4, 12, 8, 7];

        for &value in &remove_fail {
            assert!(set.swap_remove_full(&value).is_none());
        }
        println!("{:?}", set);
        for &value in &remove {
            //println!("{:?}", set);
            let index = set.get_full(&value).unwrap().0;
            assert_eq!(set.swap_remove_full(&value), Some((index, value)));
        }
        println!("{:?}", set);

        for value in &insert {
            assert_eq!(set.get(value).is_some(), !remove.contains(value));
        }
        assert_eq!(set.len(), insert.len() - remove.len());
        assert_eq!(set.iter().count(), insert.len() - remove.len());
    }

    #[test]
    fn swap_remove_index() {
        let insert = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
        let mut set = IndexSet::new();

        for &elt in &insert {
            set.insert(elt);
        }

        let mut vector = insert.to_vec();
        let remove_sequence = &[3, 3, 10, 4, 5, 4, 3, 0, 1];

        // check that the same swap remove sequence on vec and set
        // have the same result.
        for &rm in remove_sequence {
            let out_vec = vector.swap_remove(rm);
            let out_set = set.swap_remove_index(rm).unwrap();
            assert_eq!(out_vec, out_set);
        }
        assert_eq!(vector.len(), set.len());
        for (a, b) in vector.iter().zip(set.iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn partial_eq_and_eq() {
        let mut set_a = IndexSet::new();
        set_a.insert(1);
        set_a.insert(2);
        let mut set_b = set_a.clone();
        assert_eq!(set_a, set_b);
        set_b.swap_remove(&1);
        assert_ne!(set_a, set_b);

        let set_c: IndexSet<_> = set_b.into_iter().collect();
        assert_ne!(set_a, set_c);
        assert_ne!(set_c, set_a);
    }

    #[test]
    fn extend() {
        let mut set = IndexSet::new();
        set.extend(vec![&1, &2, &3, &4]);
        set.extend(vec![5, 6]);
        assert_eq!(set.into_iter().collect::<Vec<_>>(), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn comparisons() {
        let set_a: IndexSet<_> = (0..3).collect();
        let set_b: IndexSet<_> = (3..6).collect();
        let set_c: IndexSet<_> = (0..6).collect();
        let set_d: IndexSet<_> = (3..9).collect();

        assert!(!set_a.is_disjoint(&set_a));
        assert!(set_a.is_subset(&set_a));
        assert!(set_a.is_superset(&set_a));

        assert!(set_a.is_disjoint(&set_b));
        assert!(set_b.is_disjoint(&set_a));
        assert!(!set_a.is_subset(&set_b));
        assert!(!set_b.is_subset(&set_a));
        assert!(!set_a.is_superset(&set_b));
        assert!(!set_b.is_superset(&set_a));

        assert!(!set_a.is_disjoint(&set_c));
        assert!(!set_c.is_disjoint(&set_a));
        assert!(set_a.is_subset(&set_c));
        assert!(!set_c.is_subset(&set_a));
        assert!(!set_a.is_superset(&set_c));
        assert!(set_c.is_superset(&set_a));

        assert!(!set_c.is_disjoint(&set_d));
        assert!(!set_d.is_disjoint(&set_c));
        assert!(!set_c.is_subset(&set_d));
        assert!(!set_d.is_subset(&set_c));
        assert!(!set_c.is_superset(&set_d));
        assert!(!set_d.is_superset(&set_c));
    }

    #[test]
    fn iter_comparisons() {
        use std::iter::empty;

        fn check<'a, I1, I2>(iter1: I1, iter2: I2)
        where
            I1: Iterator<Item = &'a i32>,
            I2: Iterator<Item = i32>,
        {
            assert!(iter1.copied().eq(iter2));
        }

        let set_a: IndexSet<_> = (0..3).collect();
        let set_b: IndexSet<_> = (3..6).collect();
        let set_c: IndexSet<_> = (0..6).collect();
        let set_d: IndexSet<_> = (3..9).rev().collect();

        check(set_a.difference(&set_a), empty());
        check(set_a.symmetric_difference(&set_a), empty());
        check(set_a.intersection(&set_a), 0..3);
        check(set_a.union(&set_a), 0..3);

        check(set_a.difference(&set_b), 0..3);
        check(set_b.difference(&set_a), 3..6);
        check(set_a.symmetric_difference(&set_b), 0..6);
        check(set_b.symmetric_difference(&set_a), (3..6).chain(0..3));
        check(set_a.intersection(&set_b), empty());
        check(set_b.intersection(&set_a), empty());
        check(set_a.union(&set_b), 0..6);
        check(set_b.union(&set_a), (3..6).chain(0..3));

        check(set_a.difference(&set_c), empty());
        check(set_c.difference(&set_a), 3..6);
        check(set_a.symmetric_difference(&set_c), 3..6);
        check(set_c.symmetric_difference(&set_a), 3..6);
        check(set_a.intersection(&set_c), 0..3);
        check(set_c.intersection(&set_a), 0..3);
        check(set_a.union(&set_c), 0..6);
        check(set_c.union(&set_a), 0..6);

        check(set_c.difference(&set_d), 0..3);
        check(set_d.difference(&set_c), (6..9).rev());
        check(
            set_c.symmetric_difference(&set_d),
            (0..3).chain((6..9).rev()),
        );
        check(set_d.symmetric_difference(&set_c), (6..9).rev().chain(0..3));
        check(set_c.intersection(&set_d), 3..6);
        check(set_d.intersection(&set_c), (3..6).rev());
        check(set_c.union(&set_d), (0..6).chain((6..9).rev()));
        check(set_d.union(&set_c), (3..9).rev().chain(0..3));
    }

    #[test]
    fn ops() {
        let empty = IndexSet::<i32>::new();
        let set_a: IndexSet<_> = (0..3).collect();
        let set_b: IndexSet<_> = (3..6).collect();
        let set_c: IndexSet<_> = (0..6).collect();
        let set_d: IndexSet<_> = (3..9).rev().collect();

        #[allow(clippy::eq_op)]
        {
            assert_eq!(&set_a & &set_a, set_a);
            assert_eq!(&set_a | &set_a, set_a);
            assert_eq!(&set_a ^ &set_a, empty);
            assert_eq!(&set_a - &set_a, empty);
        }

        assert_eq!(&set_a & &set_b, empty);
        assert_eq!(&set_b & &set_a, empty);
        assert_eq!(&set_a | &set_b, set_c);
        assert_eq!(&set_b | &set_a, set_c);
        assert_eq!(&set_a ^ &set_b, set_c);
        assert_eq!(&set_b ^ &set_a, set_c);
        assert_eq!(&set_a - &set_b, set_a);
        assert_eq!(&set_b - &set_a, set_b);

        assert_eq!(&set_a & &set_c, set_a);
        assert_eq!(&set_c & &set_a, set_a);
        assert_eq!(&set_a | &set_c, set_c);
        assert_eq!(&set_c | &set_a, set_c);
        assert_eq!(&set_a ^ &set_c, set_b);
        assert_eq!(&set_c ^ &set_a, set_b);
        assert_eq!(&set_a - &set_c, empty);
        assert_eq!(&set_c - &set_a, set_b);

        assert_eq!(&set_c & &set_d, set_b);
        assert_eq!(&set_d & &set_c, set_b);
        assert_eq!(&set_c | &set_d, &set_a | &set_d);
        assert_eq!(&set_d | &set_c, &set_a | &set_d);
        assert_eq!(&set_c ^ &set_d, &set_a | &(&set_d - &set_b));
        assert_eq!(&set_d ^ &set_c, &set_a | &(&set_d - &set_b));
        assert_eq!(&set_c - &set_d, set_a);
        assert_eq!(&set_d - &set_c, &set_d - &set_b);
    }

    #[test]
    #[cfg(has_std)]
    fn from_array() {
        let set1 = IndexSet::from([1, 2, 3, 4]);
        let set2: IndexSet<_> = [1, 2, 3, 4].into();

        assert_eq!(set1, set2);
    }
}
