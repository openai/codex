//! [`IndexMap`] is a hash table where the iteration order of the key-value
//! pairs is independent of the hash values of the keys.

mod core;
mod iter;
mod mutable;
mod slice;

#[cfg(feature = "serde")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
pub mod serde_seq;

#[cfg(test)]
mod tests;

pub use self::core::raw_entry_v1::{self, RawEntryApiV1};
pub use self::core::{Entry, IndexedEntry, OccupiedEntry, VacantEntry};
pub use self::iter::{
    Drain, ExtractIf, IntoIter, IntoKeys, IntoValues, Iter, IterMut, IterMut2, Keys, Splice,
    Values, ValuesMut,
};
pub use self::mutable::MutableEntryKey;
pub use self::mutable::MutableKeys;
pub use self::slice::Slice;

#[cfg(feature = "rayon")]
pub use crate::rayon::map as rayon;

use ::core::cmp::Ordering;
use ::core::fmt;
use ::core::hash::{BuildHasher, Hash};
use ::core::mem;
use ::core::ops::{Index, IndexMut, RangeBounds};
use alloc::boxed::Box;
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::hash::RandomState;

pub(crate) use self::core::{ExtractCore, IndexMapCore};
use crate::util::{third, try_simplify_range};
use crate::{Bucket, Equivalent, GetDisjointMutError, HashValue, TryReserveError};

/// A hash table where the iteration order of the key-value pairs is independent
/// of the hash values of the keys.
///
/// The interface is closely compatible with the standard
/// [`HashMap`][std::collections::HashMap],
/// but also has additional features.
///
/// # Order
///
/// The key-value pairs have a consistent order that is determined by
/// the sequence of insertion and removal calls on the map. The order does
/// not depend on the keys or the hash function at all.
///
/// All iterators traverse the map in *the order*.
///
/// The insertion order is preserved, with **notable exceptions** like the
/// [`.remove()`][Self::remove] or [`.swap_remove()`][Self::swap_remove] methods.
/// Methods such as [`.sort_by()`][Self::sort_by] of
/// course result in a new order, depending on the sorting order.
///
/// # Indices
///
/// The key-value pairs are indexed in a compact range without holes in the
/// range `0..self.len()`. For example, the method `.get_full` looks up the
/// index for a key, and the method `.get_index` looks up the key-value pair by
/// index.
///
/// # Examples
///
/// ```
/// use indexmap::IndexMap;
///
/// // count the frequency of each letter in a sentence.
/// let mut letters = IndexMap::new();
/// for ch in "a short treatise on fungi".chars() {
///     *letters.entry(ch).or_insert(0) += 1;
/// }
///
/// assert_eq!(letters[&'s'], 2);
/// assert_eq!(letters[&'t'], 3);
/// assert_eq!(letters[&'u'], 1);
/// assert_eq!(letters.get(&'y'), None);
/// ```
#[cfg(feature = "std")]
pub struct IndexMap<K, V, S = RandomState> {
    pub(crate) core: IndexMapCore<K, V>,
    hash_builder: S,
}
#[cfg(not(feature = "std"))]
pub struct IndexMap<K, V, S> {
    pub(crate) core: IndexMapCore<K, V>,
    hash_builder: S,
}

impl<K, V, S> Clone for IndexMap<K, V, S>
where
    K: Clone,
    V: Clone,
    S: Clone,
{
    fn clone(&self) -> Self {
        IndexMap {
            core: self.core.clone(),
            hash_builder: self.hash_builder.clone(),
        }
    }

    fn clone_from(&mut self, other: &Self) {
        self.core.clone_from(&other.core);
        self.hash_builder.clone_from(&other.hash_builder);
    }
}

impl<K, V, S> fmt::Debug for IndexMap<K, V, S>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    #[cfg(not(feature = "test_debug"))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }

    #[cfg(feature = "test_debug")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Let the inner `IndexMapCore` print all of its details
        f.debug_struct("IndexMap")
            .field("core", &self.core)
            .finish()
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<K, V> IndexMap<K, V> {
    /// Create a new map. (Does not allocate.)
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Create a new map with capacity for `n` key-value pairs. (Does not
    /// allocate if `n` is zero.)
    ///
    /// Computes in **O(n)** time.
    #[inline]
    pub fn with_capacity(n: usize) -> Self {
        Self::with_capacity_and_hasher(n, <_>::default())
    }
}

impl<K, V, S> IndexMap<K, V, S> {
    /// Create a new map with capacity for `n` key-value pairs. (Does not
    /// allocate if `n` is zero.)
    ///
    /// Computes in **O(n)** time.
    #[inline]
    pub fn with_capacity_and_hasher(n: usize, hash_builder: S) -> Self {
        if n == 0 {
            Self::with_hasher(hash_builder)
        } else {
            IndexMap {
                core: IndexMapCore::with_capacity(n),
                hash_builder,
            }
        }
    }

    /// Create a new map with `hash_builder`.
    ///
    /// This function is `const`, so it
    /// can be called in `static` contexts.
    pub const fn with_hasher(hash_builder: S) -> Self {
        IndexMap {
            core: IndexMapCore::new(),
            hash_builder,
        }
    }

    #[inline]
    pub(crate) fn into_entries(self) -> Vec<Bucket<K, V>> {
        self.core.into_entries()
    }

    #[inline]
    pub(crate) fn as_entries(&self) -> &[Bucket<K, V>] {
        self.core.as_entries()
    }

    #[inline]
    pub(crate) fn as_entries_mut(&mut self) -> &mut [Bucket<K, V>] {
        self.core.as_entries_mut()
    }

    pub(crate) fn with_entries<F>(&mut self, f: F)
    where
        F: FnOnce(&mut [Bucket<K, V>]),
    {
        self.core.with_entries(f);
    }

    /// Return the number of elements the map can hold without reallocating.
    ///
    /// This number is a lower bound; the map might be able to hold more,
    /// but is guaranteed to be able to hold at least this many.
    ///
    /// Computes in **O(1)** time.
    pub fn capacity(&self) -> usize {
        self.core.capacity()
    }

    /// Return a reference to the map's `BuildHasher`.
    pub fn hasher(&self) -> &S {
        &self.hash_builder
    }

    /// Return the number of key-value pairs in the map.
    ///
    /// Computes in **O(1)** time.
    #[inline]
    pub fn len(&self) -> usize {
        self.core.len()
    }

    /// Returns true if the map contains no elements.
    ///
    /// Computes in **O(1)** time.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return an iterator over the key-value pairs of the map, in their order
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter::new(self.as_entries())
    }

    /// Return an iterator over the key-value pairs of the map, in their order
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut::new(self.as_entries_mut())
    }

    /// Return an iterator over the keys of the map, in their order
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys::new(self.as_entries())
    }

    /// Return an owning iterator over the keys of the map, in their order
    pub fn into_keys(self) -> IntoKeys<K, V> {
        IntoKeys::new(self.into_entries())
    }

    /// Return an iterator over the values of the map, in their order
    pub fn values(&self) -> Values<'_, K, V> {
        Values::new(self.as_entries())
    }

    /// Return an iterator over mutable references to the values of the map,
    /// in their order
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        ValuesMut::new(self.as_entries_mut())
    }

    /// Return an owning iterator over the values of the map, in their order
    pub fn into_values(self) -> IntoValues<K, V> {
        IntoValues::new(self.into_entries())
    }

    /// Remove all key-value pairs in the map, while preserving its capacity.
    ///
    /// Computes in **O(n)** time.
    pub fn clear(&mut self) {
        self.core.clear();
    }

    /// Shortens the map, keeping the first `len` elements and dropping the rest.
    ///
    /// If `len` is greater than the map's current length, this has no effect.
    pub fn truncate(&mut self, len: usize) {
        self.core.truncate(len);
    }

    /// Clears the `IndexMap` in the given index range, returning those
    /// key-value pairs as a drain iterator.
    ///
    /// The range may be any type that implements [`RangeBounds<usize>`],
    /// including all of the `std::ops::Range*` types, or even a tuple pair of
    /// `Bound` start and end values. To drain the map entirely, use `RangeFull`
    /// like `map.drain(..)`.
    ///
    /// This shifts down all entries following the drained range to fill the
    /// gap, and keeps the allocated memory for reuse.
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the map.
    #[track_caller]
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, K, V>
    where
        R: RangeBounds<usize>,
    {
        Drain::new(self.core.drain(range))
    }

    /// Creates an iterator which uses a closure to determine if an element should be removed,
    /// for all elements in the given range.
    ///
    /// If the closure returns true, the element is removed from the map and yielded.
    /// If the closure returns false, or panics, the element remains in the map and will not be
    /// yielded.
    ///
    /// Note that `extract_if` lets you mutate every value in the filter closure, regardless of
    /// whether you choose to keep or remove it.
    ///
    /// The range may be any type that implements [`RangeBounds<usize>`],
    /// including all of the `std::ops::Range*` types, or even a tuple pair of
    /// `Bound` start and end values. To check the entire map, use `RangeFull`
    /// like `map.extract_if(.., predicate)`.
    ///
    /// If the returned `ExtractIf` is not exhausted, e.g. because it is dropped without iterating
    /// or the iteration short-circuits, then the remaining elements will be retained.
    /// Use [`retain`] with a negated predicate if you do not need the returned iterator.
    ///
    /// [`retain`]: IndexMap::retain
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the map.
    ///
    /// # Examples
    ///
    /// Splitting a map into even and odd keys, reusing the original map:
    ///
    /// ```
    /// use indexmap::IndexMap;
    ///
    /// let mut map: IndexMap<i32, i32> = (0..8).map(|x| (x, x)).collect();
    /// let extracted: IndexMap<i32, i32> = map.extract_if(.., |k, _v| k % 2 == 0).collect();
    ///
    /// let evens = extracted.keys().copied().collect::<Vec<_>>();
    /// let odds = map.keys().copied().collect::<Vec<_>>();
    ///
    /// assert_eq!(evens, vec![0, 2, 4, 6]);
    /// assert_eq!(odds, vec![1, 3, 5, 7]);
    /// ```
    #[track_caller]
    pub fn extract_if<F, R>(&mut self, range: R, pred: F) -> ExtractIf<'_, K, V, F>
    where
        F: FnMut(&K, &mut V) -> bool,
        R: RangeBounds<usize>,
    {
        ExtractIf::new(&mut self.core, range, pred)
    }

    /// Splits the collection into two at the given index.
    ///
    /// Returns a newly allocated map containing the elements in the range
    /// `[at, len)`. After the call, the original map will be left containing
    /// the elements `[0, at)` with its previous capacity unchanged.
    ///
    /// ***Panics*** if `at > len`.
    #[track_caller]
    pub fn split_off(&mut self, at: usize) -> Self
    where
        S: Clone,
    {
        Self {
            core: self.core.split_off(at),
            hash_builder: self.hash_builder.clone(),
        }
    }

    /// Reserve capacity for `additional` more key-value pairs.
    ///
    /// Computes in **O(n)** time.
    pub fn reserve(&mut self, additional: usize) {
        self.core.reserve(additional);
    }

    /// Reserve capacity for `additional` more key-value pairs, without over-allocating.
    ///
    /// Unlike `reserve`, this does not deliberately over-allocate the entry capacity to avoid
    /// frequent re-allocations. However, the underlying data structures may still have internal
    /// capacity requirements, and the allocator itself may give more space than requested, so this
    /// cannot be relied upon to be precisely minimal.
    ///
    /// Computes in **O(n)** time.
    pub fn reserve_exact(&mut self, additional: usize) {
        self.core.reserve_exact(additional);
    }

    /// Try to reserve capacity for `additional` more key-value pairs.
    ///
    /// Computes in **O(n)** time.
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.core.try_reserve(additional)
    }

    /// Try to reserve capacity for `additional` more key-value pairs, without over-allocating.
    ///
    /// Unlike `try_reserve`, this does not deliberately over-allocate the entry capacity to avoid
    /// frequent re-allocations. However, the underlying data structures may still have internal
    /// capacity requirements, and the allocator itself may give more space than requested, so this
    /// cannot be relied upon to be precisely minimal.
    ///
    /// Computes in **O(n)** time.
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.core.try_reserve_exact(additional)
    }

    /// Shrink the capacity of the map as much as possible.
    ///
    /// Computes in **O(n)** time.
    pub fn shrink_to_fit(&mut self) {
        self.core.shrink_to(0);
    }

    /// Shrink the capacity of the map with a lower limit.
    ///
    /// Computes in **O(n)** time.
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.core.shrink_to(min_capacity);
    }
}

impl<K, V, S> IndexMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    /// Insert a key-value pair in the map.
    ///
    /// If an equivalent key already exists in the map: the key remains and
    /// retains in its place in the order, its corresponding value is updated
    /// with `value`, and the older value is returned inside `Some(_)`.
    ///
    /// If no equivalent key existed in the map: the new key-value pair is
    /// inserted, last in order, and `None` is returned.
    ///
    /// Computes in **O(1)** time (amortized average).
    ///
    /// See also [`entry`][Self::entry] if you want to insert *or* modify,
    /// or [`insert_full`][Self::insert_full] if you need to get the index of
    /// the corresponding key-value pair.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert_full(key, value).1
    }

    /// Insert a key-value pair in the map, and get their index.
    ///
    /// If an equivalent key already exists in the map: the key remains and
    /// retains in its place in the order, its corresponding value is updated
    /// with `value`, and the older value is returned inside `(index, Some(_))`.
    ///
    /// If no equivalent key existed in the map: the new key-value pair is
    /// inserted, last in order, and `(index, None)` is returned.
    ///
    /// Computes in **O(1)** time (amortized average).
    ///
    /// See also [`entry`][Self::entry] if you want to insert *or* modify.
    pub fn insert_full(&mut self, key: K, value: V) -> (usize, Option<V>) {
        let hash = self.hash(&key);
        self.core.insert_full(hash, key, value)
    }

    /// Insert a key-value pair in the map at its ordered position among sorted keys.
    ///
    /// This is equivalent to finding the position with
    /// [`binary_search_keys`][Self::binary_search_keys], then either updating
    /// it or calling [`insert_before`][Self::insert_before] for a new key.
    ///
    /// If the sorted key is found in the map, its corresponding value is
    /// updated with `value`, and the older value is returned inside
    /// `(index, Some(_))`. Otherwise, the new key-value pair is inserted at
    /// the sorted position, and `(index, None)` is returned.
    ///
    /// If the existing keys are **not** already sorted, then the insertion
    /// index is unspecified (like [`slice::binary_search`]), but the key-value
    /// pair is moved to or inserted at that position regardless.
    ///
    /// Computes in **O(n)** time (average). Instead of repeating calls to
    /// `insert_sorted`, it may be faster to call batched [`insert`][Self::insert]
    /// or [`extend`][Self::extend] and only call [`sort_keys`][Self::sort_keys]
    /// or [`sort_unstable_keys`][Self::sort_unstable_keys] once.
    pub fn insert_sorted(&mut self, key: K, value: V) -> (usize, Option<V>)
    where
        K: Ord,
    {
        match self.binary_search_keys(&key) {
            Ok(i) => (i, Some(mem::replace(&mut self[i], value))),
            Err(i) => self.insert_before(i, key, value),
        }
    }

    /// Insert a key-value pair in the map at its ordered position among keys
    /// sorted by `cmp`.
    ///
    /// This is equivalent to finding the position with
    /// [`binary_search_by`][Self::binary_search_by], then calling
    /// [`insert_before`][Self::insert_before] with the given key and value.
    ///
    /// If the existing keys are **not** already sorted, then the insertion
    /// index is unspecified (like [`slice::binary_search`]), but the key-value
    /// pair is moved to or inserted at that position regardless.
    ///
    /// Computes in **O(n)** time (average).
    pub fn insert_sorted_by<F>(&mut self, key: K, value: V, mut cmp: F) -> (usize, Option<V>)
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        let (Ok(i) | Err(i)) = self.binary_search_by(|k, v| cmp(k, v, &key, &value));
        self.insert_before(i, key, value)
    }

    /// Insert a key-value pair in the map at its ordered position
    /// using a sort-key extraction function.
    ///
    /// This is equivalent to finding the position with
    /// [`binary_search_by_key`][Self::binary_search_by_key] with `sort_key(key)`, then
    /// calling [`insert_before`][Self::insert_before] with the given key and value.
    ///
    /// If the existing keys are **not** already sorted, then the insertion
    /// index is unspecified (like [`slice::binary_search`]), but the key-value
    /// pair is moved to or inserted at that position regardless.
    ///
    /// Computes in **O(n)** time (average).
    pub fn insert_sorted_by_key<B, F>(
        &mut self,
        key: K,
        value: V,
        mut sort_key: F,
    ) -> (usize, Option<V>)
    where
        B: Ord,
        F: FnMut(&K, &V) -> B,
    {
        let search_key = sort_key(&key, &value);
        let (Ok(i) | Err(i)) = self.binary_search_by_key(&search_key, sort_key);
        self.insert_before(i, key, value)
    }

    /// Insert a key-value pair in the map before the entry at the given index, or at the end.
    ///
    /// If an equivalent key already exists in the map: the key remains and
    /// is moved to the new position in the map, its corresponding value is updated
    /// with `value`, and the older value is returned inside `Some(_)`. The returned index
    /// will either be the given index or one less, depending on how the entry moved.
    /// (See [`shift_insert`](Self::shift_insert) for different behavior here.)
    ///
    /// If no equivalent key existed in the map: the new key-value pair is
    /// inserted exactly at the given index, and `None` is returned.
    ///
    /// ***Panics*** if `index` is out of bounds.
    /// Valid indices are `0..=map.len()` (inclusive).
    ///
    /// Computes in **O(n)** time (average).
    ///
    /// See also [`entry`][Self::entry] if you want to insert *or* modify,
    /// perhaps only using the index for new entries with [`VacantEntry::shift_insert`].
    ///
    /// # Examples
    ///
    /// ```
    /// use indexmap::IndexMap;
    /// let mut map: IndexMap<char, ()> = ('a'..='z').map(|c| (c, ())).collect();
    ///
    /// // The new key '*' goes exactly at the given index.
    /// assert_eq!(map.get_index_of(&'*'), None);
    /// assert_eq!(map.insert_before(10, '*', ()), (10, None));
    /// assert_eq!(map.get_index_of(&'*'), Some(10));
    ///
    /// // Moving the key 'a' up will shift others down, so this moves *before* 10 to index 9.
    /// assert_eq!(map.insert_before(10, 'a', ()), (9, Some(())));
    /// assert_eq!(map.get_index_of(&'a'), Some(9));
    /// assert_eq!(map.get_index_of(&'*'), Some(10));
    ///
    /// // Moving the key 'z' down will shift others up, so this moves to exactly 10.
    /// assert_eq!(map.insert_before(10, 'z', ()), (10, Some(())));
    /// assert_eq!(map.get_index_of(&'z'), Some(10));
    /// assert_eq!(map.get_index_of(&'*'), Some(11));
    ///
    /// // Moving or inserting before the endpoint is also valid.
    /// assert_eq!(map.len(), 27);
    /// assert_eq!(map.insert_before(map.len(), '*', ()), (26, Some(())));
    /// assert_eq!(map.get_index_of(&'*'), Some(26));
    /// assert_eq!(map.insert_before(map.len(), '+', ()), (27, None));
    /// assert_eq!(map.get_index_of(&'+'), Some(27));
    /// assert_eq!(map.len(), 28);
    /// ```
    #[track_caller]
    pub fn insert_before(&mut self, mut index: usize, key: K, value: V) -> (usize, Option<V>) {
        let len = self.len();

        assert!(
            index <= len,
            "index out of bounds: the len is {len} but the index is {index}. Expected index <= len"
        );

        match self.entry(key) {
            Entry::Occupied(mut entry) => {
                if index > entry.index() {
                    // Some entries will shift down when this one moves up,
                    // so "insert before index" becomes "move to index - 1",
                    // keeping the entry at the original index unmoved.
                    index -= 1;
                }
                let old = mem::replace(entry.get_mut(), value);
                entry.move_index(index);
                (index, Some(old))
            }
            Entry::Vacant(entry) => {
                entry.shift_insert(index, value);
                (index, None)
            }
        }
    }

    /// Insert a key-value pair in the map at the given index.
    ///
    /// If an equivalent key already exists in the map: the key remains and
    /// is moved to the given index in the map, its corresponding value is updated
    /// with `value`, and the older value is returned inside `Some(_)`.
    /// Note that existing entries **cannot** be moved to `index == map.len()`!
    /// (See [`insert_before`](Self::insert_before) for different behavior here.)
    ///
    /// If no equivalent key existed in the map: the new key-value pair is
    /// inserted at the given index, and `None` is returned.
    ///
    /// ***Panics*** if `index` is out of bounds.
    /// Valid indices are `0..map.len()` (exclusive) when moving an existing entry, or
    /// `0..=map.len()` (inclusive) when inserting a new key.
    ///
    /// Computes in **O(n)** time (average).
    ///
    /// See also [`entry`][Self::entry] if you want to insert *or* modify,
    /// perhaps only using the index for new entries with [`VacantEntry::shift_insert`].
    ///
    /// # Examples
    ///
    /// ```
    /// use indexmap::IndexMap;
    /// let mut map: IndexMap<char, ()> = ('a'..='z').map(|c| (c, ())).collect();
    ///
    /// // The new key '*' goes exactly at the given index.
    /// assert_eq!(map.get_index_of(&'*'), None);
    /// assert_eq!(map.shift_insert(10, '*', ()), None);
    /// assert_eq!(map.get_index_of(&'*'), Some(10));
    ///
    /// // Moving the key 'a' up to 10 will shift others down, including the '*' that was at 10.
    /// assert_eq!(map.shift_insert(10, 'a', ()), Some(()));
    /// assert_eq!(map.get_index_of(&'a'), Some(10));
    /// assert_eq!(map.get_index_of(&'*'), Some(9));
    ///
    /// // Moving the key 'z' down to 9 will shift others up, including the '*' that was at 9.
    /// assert_eq!(map.shift_insert(9, 'z', ()), Some(()));
    /// assert_eq!(map.get_index_of(&'z'), Some(9));
    /// assert_eq!(map.get_index_of(&'*'), Some(10));
    ///
    /// // Existing keys can move to len-1 at most, but new keys can insert at the endpoint.
    /// assert_eq!(map.len(), 27);
    /// assert_eq!(map.shift_insert(map.len() - 1, '*', ()), Some(()));
    /// assert_eq!(map.get_index_of(&'*'), Some(26));
    /// assert_eq!(map.shift_insert(map.len(), '+', ()), None);
    /// assert_eq!(map.get_index_of(&'+'), Some(27));
    /// assert_eq!(map.len(), 28);
    /// ```
    ///
    /// ```should_panic
    /// use indexmap::IndexMap;
    /// let mut map: IndexMap<char, ()> = ('a'..='z').map(|c| (c, ())).collect();
    ///
    /// // This is an invalid index for moving an existing key!
    /// map.shift_insert(map.len(), 'a', ());
    /// ```
    #[track_caller]
    pub fn shift_insert(&mut self, index: usize, key: K, value: V) -> Option<V> {
        let len = self.len();
        match self.entry(key) {
            Entry::Occupied(mut entry) => {
                assert!(
                    index < len,
                    "index out of bounds: the len is {len} but the index is {index}"
                );

                let old = mem::replace(entry.get_mut(), value);
                entry.move_index(index);
                Some(old)
            }
            Entry::Vacant(entry) => {
                assert!(
                    index <= len,
                    "index out of bounds: the len is {len} but the index is {index}. Expected index <= len"
                );

                entry.shift_insert(index, value);
                None
            }
        }
    }

    /// Replaces the key at the given index. The new key does not need to be
    /// equivalent to the one it is replacing, but it must be unique to the rest
    /// of the map.
    ///
    /// Returns `Ok(old_key)` if successful, or `Err((other_index, key))` if an
    /// equivalent key already exists at a different index. The map will be
    /// unchanged in the error case.
    ///
    /// Direct indexing can be used to change the corresponding value: simply
    /// `map[index] = value`, or `mem::replace(&mut map[index], value)` to
    /// retrieve the old value as well.
    ///
    /// ***Panics*** if `index` is out of bounds.
    ///
    /// Computes in **O(1)** time (average).
    #[track_caller]
    pub fn replace_index(&mut self, index: usize, key: K) -> Result<K, (usize, K)> {
        // If there's a direct match, we don't even need to hash it.
        let entry = &mut self.as_entries_mut()[index];
        if key == entry.key {
            return Ok(mem::replace(&mut entry.key, key));
        }

        let hash = self.hash(&key);
        if let Some(i) = self.core.get_index_of(hash, &key) {
            debug_assert_ne!(i, index);
            return Err((i, key));
        }
        Ok(self.core.replace_index_unique(index, hash, key))
    }

    /// Get the given key's corresponding entry in the map for insertion and/or
    /// in-place manipulation.
    ///
    /// Computes in **O(1)** time (amortized average).
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        let hash = self.hash(&key);
        self.core.entry(hash, key)
    }

    /// Creates a splicing iterator that replaces the specified range in the map
    /// with the given `replace_with` key-value iterator and yields the removed
    /// items. `replace_with` does not need to be the same length as `range`.
    ///
    /// The `range` is removed even if the iterator is not consumed until the
    /// end. It is unspecified how many elements are removed from the map if the
    /// `Splice` value is leaked.
    ///
    /// The input iterator `replace_with` is only consumed when the `Splice`
    /// value is dropped. If a key from the iterator matches an existing entry
    /// in the map (outside of `range`), then the value will be updated in that
    /// position. Otherwise, the new key-value pair will be inserted in the
    /// replaced `range`.
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use indexmap::IndexMap;
    ///
    /// let mut map = IndexMap::from([(0, '_'), (1, 'a'), (2, 'b'), (3, 'c'), (4, 'd')]);
    /// let new = [(5, 'E'), (4, 'D'), (3, 'C'), (2, 'B'), (1, 'A')];
    /// let removed: Vec<_> = map.splice(2..4, new).collect();
    ///
    /// // 1 and 4 got new values, while 5, 3, and 2 were newly inserted.
    /// assert!(map.into_iter().eq([(0, '_'), (1, 'A'), (5, 'E'), (3, 'C'), (2, 'B'), (4, 'D')]));
    /// assert_eq!(removed, &[(2, 'b'), (3, 'c')]);
    /// ```
    #[track_caller]
    pub fn splice<R, I>(&mut self, range: R, replace_with: I) -> Splice<'_, I::IntoIter, K, V, S>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = (K, V)>,
    {
        Splice::new(self, range, replace_with.into_iter())
    }

    /// Moves all key-value pairs from `other` into `self`, leaving `other` empty.
    ///
    /// This is equivalent to calling [`insert`][Self::insert] for each
    /// key-value pair from `other` in order, which means that for keys that
    /// already exist in `self`, their value is updated in the current position.
    ///
    /// # Examples
    ///
    /// ```
    /// use indexmap::IndexMap;
    ///
    /// // Note: Key (3) is present in both maps.
    /// let mut a = IndexMap::from([(3, "c"), (2, "b"), (1, "a")]);
    /// let mut b = IndexMap::from([(3, "d"), (4, "e"), (5, "f")]);
    /// let old_capacity = b.capacity();
    ///
    /// a.append(&mut b);
    ///
    /// assert_eq!(a.len(), 5);
    /// assert_eq!(b.len(), 0);
    /// assert_eq!(b.capacity(), old_capacity);
    ///
    /// assert!(a.keys().eq(&[3, 2, 1, 4, 5]));
    /// assert_eq!(a[&3], "d"); // "c" was overwritten.
    /// ```
    pub fn append<S2>(&mut self, other: &mut IndexMap<K, V, S2>) {
        self.extend(other.drain(..));
    }
}

impl<K, V, S> IndexMap<K, V, S>
where
    S: BuildHasher,
{
    pub(crate) fn hash<Q: ?Sized + Hash>(&self, key: &Q) -> HashValue {
        let h = self.hash_builder.hash_one(key);
        HashValue(h as usize)
    }

    /// Return `true` if an equivalent to `key` exists in the map.
    ///
    /// Computes in **O(1)** time (average).
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.get_index_of(key).is_some()
    }

    /// Return a reference to the stored value for `key`, if it is present,
    /// else `None`.
    ///
    /// Computes in **O(1)** time (average).
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &self.as_entries()[i];
            Some(&entry.value)
        } else {
            None
        }
    }

    /// Return references to the stored key-value pair for the lookup `key`,
    /// if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&K, &V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &self.as_entries()[i];
            Some((&entry.key, &entry.value))
        } else {
            None
        }
    }

    /// Return the index with references to the stored key-value pair for the
    /// lookup `key`, if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    pub fn get_full<Q>(&self, key: &Q) -> Option<(usize, &K, &V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &self.as_entries()[i];
            Some((i, &entry.key, &entry.value))
        } else {
            None
        }
    }

    /// Return the item index for `key`, if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    pub fn get_index_of<Q>(&self, key: &Q) -> Option<usize>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        match self.as_entries() {
            [] => None,
            [x] => key.equivalent(&x.key).then_some(0),
            _ => {
                let hash = self.hash(key);
                self.core.get_index_of(hash, key)
            }
        }
    }

    /// Return a mutable reference to the stored value for `key`,
    /// if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &mut self.as_entries_mut()[i];
            Some(&mut entry.value)
        } else {
            None
        }
    }

    /// Return a reference and mutable references to the stored key-value pair
    /// for the lookup `key`, if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    pub fn get_key_value_mut<Q>(&mut self, key: &Q) -> Option<(&K, &mut V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &mut self.as_entries_mut()[i];
            Some((&entry.key, &mut entry.value))
        } else {
            None
        }
    }

    /// Return the index with a reference and mutable reference to the stored
    /// key-value pair for the lookup `key`, if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    pub fn get_full_mut<Q>(&mut self, key: &Q) -> Option<(usize, &K, &mut V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &mut self.as_entries_mut()[i];
            Some((i, &entry.key, &mut entry.value))
        } else {
            None
        }
    }

    /// Return the values for `N` keys. If any key is duplicated, this function will panic.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut map = indexmap::IndexMap::from([(1, 'a'), (3, 'b'), (2, 'c')]);
    /// assert_eq!(map.get_disjoint_mut([&2, &1]), [Some(&mut 'c'), Some(&mut 'a')]);
    /// ```
    pub fn get_disjoint_mut<Q, const N: usize>(&mut self, keys: [&Q; N]) -> [Option<&mut V>; N]
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        let indices = keys.map(|key| self.get_index_of(key));
        match self.as_mut_slice().get_disjoint_opt_mut(indices) {
            Err(GetDisjointMutError::IndexOutOfBounds) => {
                unreachable!(
                    "Internal error: indices should never be OOB as we got them from get_index_of"
                );
            }
            Err(GetDisjointMutError::OverlappingIndices) => {
                panic!("duplicate keys found");
            }
            Ok(key_values) => key_values.map(|kv_opt| kv_opt.map(|kv| kv.1)),
        }
    }

    /// Remove the key-value pair equivalent to `key` and return
    /// its value.
    ///
    /// **NOTE:** This is equivalent to [`.swap_remove(key)`][Self::swap_remove], replacing this
    /// entry's position with the last element, and it is deprecated in favor of calling that
    /// explicitly. If you need to preserve the relative order of the keys in the map, use
    /// [`.shift_remove(key)`][Self::shift_remove] instead.
    #[deprecated(note = "`remove` disrupts the map order -- \
        use `swap_remove` or `shift_remove` for explicit behavior.")]
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.swap_remove(key)
    }

    /// Remove and return the key-value pair equivalent to `key`.
    ///
    /// **NOTE:** This is equivalent to [`.swap_remove_entry(key)`][Self::swap_remove_entry],
    /// replacing this entry's position with the last element, and it is deprecated in favor of
    /// calling that explicitly. If you need to preserve the relative order of the keys in the map,
    /// use [`.shift_remove_entry(key)`][Self::shift_remove_entry] instead.
    #[deprecated(note = "`remove_entry` disrupts the map order -- \
        use `swap_remove_entry` or `shift_remove_entry` for explicit behavior.")]
    pub fn remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.swap_remove_entry(key)
    }

    /// Remove the key-value pair equivalent to `key` and return
    /// its value.
    ///
    /// Like [`Vec::swap_remove`], the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.swap_remove_full(key).map(third)
    }

    /// Remove and return the key-value pair equivalent to `key`.
    ///
    /// Like [`Vec::swap_remove`], the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        match self.swap_remove_full(key) {
            Some((_, key, value)) => Some((key, value)),
            None => None,
        }
    }

    /// Remove the key-value pair equivalent to `key` and return it and
    /// the index it had.
    ///
    /// Like [`Vec::swap_remove`], the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove_full<Q>(&mut self, key: &Q) -> Option<(usize, K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        match self.as_entries() {
            [x] if key.equivalent(&x.key) => {
                let (k, v) = self.core.pop()?;
                Some((0, k, v))
            }
            [_] | [] => None,
            _ => {
                let hash = self.hash(key);
                self.core.swap_remove_full(hash, key)
            }
        }
    }

    /// Remove the key-value pair equivalent to `key` and return
    /// its value.
    ///
    /// Like [`Vec::remove`], the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.shift_remove_full(key).map(third)
    }

    /// Remove and return the key-value pair equivalent to `key`.
    ///
    /// Like [`Vec::remove`], the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        match self.shift_remove_full(key) {
            Some((_, key, value)) => Some((key, value)),
            None => None,
        }
    }

    /// Remove the key-value pair equivalent to `key` and return it and
    /// the index it had.
    ///
    /// Like [`Vec::remove`], the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove_full<Q>(&mut self, key: &Q) -> Option<(usize, K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        match self.as_entries() {
            [x] if key.equivalent(&x.key) => {
                let (k, v) = self.core.pop()?;
                Some((0, k, v))
            }
            [_] | [] => None,
            _ => {
                let hash = self.hash(key);
                self.core.shift_remove_full(hash, key)
            }
        }
    }
}

impl<K, V, S> IndexMap<K, V, S> {
    /// Remove the last key-value pair
    ///
    /// This preserves the order of the remaining elements.
    ///
    /// Computes in **O(1)** time (average).
    #[doc(alias = "pop_last")] // like `BTreeMap`
    pub fn pop(&mut self) -> Option<(K, V)> {
        self.core.pop()
    }

    /// Removes and returns the last key-value pair from a map if the predicate
    /// returns `true`, or [`None`] if the predicate returns false or the map
    /// is empty (the predicate will not be called in that case).
    ///
    /// This preserves the order of the remaining elements.
    ///
    /// Computes in **O(1)** time (average).
    ///
    /// # Examples
    ///
    /// ```
    /// use indexmap::IndexMap;
    ///
    /// let init = [(1, 'a'), (2, 'b'), (3, 'c'), (4, 'd')];
    /// let mut map = IndexMap::from(init);
    /// let pred = |key: &i32, _value: &mut char| *key % 2 == 0;
    ///
    /// assert_eq!(map.pop_if(pred), Some((4, 'd')));
    /// assert_eq!(map.as_slice(), &init[..3]);
    /// assert_eq!(map.pop_if(pred), None);
    /// ```
    pub fn pop_if(&mut self, predicate: impl FnOnce(&K, &mut V) -> bool) -> Option<(K, V)> {
        let (last_key, last_value) = self.last_mut()?;
        if predicate(last_key, last_value) {
            self.core.pop()
        } else {
            None
        }
    }

    /// Scan through each key-value pair in the map and keep those where the
    /// closure `keep` returns `true`.
    ///
    /// The elements are visited in order, and remaining elements keep their
    /// order.
    ///
    /// Computes in **O(n)** time (average).
    pub fn retain<F>(&mut self, mut keep: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        self.core.retain_in_order(move |k, v| keep(k, v));
    }

    /// Sort the map's key-value pairs by the default ordering of the keys.
    ///
    /// This is a stable sort -- but equivalent keys should not normally coexist in
    /// a map at all, so [`sort_unstable_keys`][Self::sort_unstable_keys] is preferred
    /// because it is generally faster and doesn't allocate auxiliary memory.
    ///
    /// See [`sort_by`](Self::sort_by) for details.
    pub fn sort_keys(&mut self)
    where
        K: Ord,
    {
        self.with_entries(move |entries| {
            entries.sort_by(move |a, b| K::cmp(&a.key, &b.key));
        });
    }

    /// Sort the map's key-value pairs in place using the comparison
    /// function `cmp`.
    ///
    /// The comparison function receives two key and value pairs to compare (you
    /// can sort by keys or values or their combination as needed).
    ///
    /// Computes in **O(n log n + c)** time and **O(n)** space where *n* is
    /// the length of the map and *c* the capacity. The sort is stable.
    pub fn sort_by<F>(&mut self, mut cmp: F)
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        self.with_entries(move |entries| {
            entries.sort_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value));
        });
    }

    /// Sort the key-value pairs of the map and return a by-value iterator of
    /// the key-value pairs with the result.
    ///
    /// The sort is stable.
    pub fn sorted_by<F>(self, mut cmp: F) -> IntoIter<K, V>
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        let mut entries = self.into_entries();
        entries.sort_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value));
        IntoIter::new(entries)
    }

    /// Sort the map's key-value pairs in place using a sort-key extraction function.
    ///
    /// Computes in **O(n log n + c)** time and **O(n)** space where *n* is
    /// the length of the map and *c* the capacity. The sort is stable.
    pub fn sort_by_key<T, F>(&mut self, mut sort_key: F)
    where
        T: Ord,
        F: FnMut(&K, &V) -> T,
    {
        self.with_entries(move |entries| {
            entries.sort_by_key(move |a| sort_key(&a.key, &a.value));
        });
    }

    /// Sort the map's key-value pairs by the default ordering of the keys, but
    /// may not preserve the order of equal elements.
    ///
    /// See [`sort_unstable_by`](Self::sort_unstable_by) for details.
    pub fn sort_unstable_keys(&mut self)
    where
        K: Ord,
    {
        self.with_entries(move |entries| {
            entries.sort_unstable_by(move |a, b| K::cmp(&a.key, &b.key));
        });
    }

    /// Sort the map's key-value pairs in place using the comparison function `cmp`, but
    /// may not preserve the order of equal elements.
    ///
    /// The comparison function receives two key and value pairs to compare (you
    /// can sort by keys or values or their combination as needed).
    ///
    /// Computes in **O(n log n + c)** time where *n* is
    /// the length of the map and *c* is the capacity. The sort is unstable.
    pub fn sort_unstable_by<F>(&mut self, mut cmp: F)
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        self.with_entries(move |entries| {
            entries.sort_unstable_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value));
        });
    }

    /// Sort the key-value pairs of the map and return a by-value iterator of
    /// the key-value pairs with the result.
    ///
    /// The sort is unstable.
    #[inline]
    pub fn sorted_unstable_by<F>(self, mut cmp: F) -> IntoIter<K, V>
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        let mut entries = self.into_entries();
        entries.sort_unstable_by(move |a, b| cmp(&a.key, &a.value, &b.key, &b.value));
        IntoIter::new(entries)
    }

    /// Sort the map's key-value pairs in place using a sort-key extraction function.
    ///
    /// Computes in **O(n log n + c)** time where *n* is
    /// the length of the map and *c* is the capacity. The sort is unstable.
    pub fn sort_unstable_by_key<T, F>(&mut self, mut sort_key: F)
    where
        T: Ord,
        F: FnMut(&K, &V) -> T,
    {
        self.with_entries(move |entries| {
            entries.sort_unstable_by_key(move |a| sort_key(&a.key, &a.value));
        });
    }

    /// Sort the map's key-value pairs in place using a sort-key extraction function.
    ///
    /// During sorting, the function is called at most once per entry, by using temporary storage
    /// to remember the results of its evaluation. The order of calls to the function is
    /// unspecified and may change between versions of `indexmap` or the standard library.
    ///
    /// Computes in **O(m n + n log n + c)** time () and **O(n)** space, where the function is
    /// **O(m)**, *n* is the length of the map, and *c* the capacity. The sort is stable.
    pub fn sort_by_cached_key<T, F>(&mut self, mut sort_key: F)
    where
        T: Ord,
        F: FnMut(&K, &V) -> T,
    {
        self.with_entries(move |entries| {
            entries.sort_by_cached_key(move |a| sort_key(&a.key, &a.value));
        });
    }

    /// Search over a sorted map for a key.
    ///
    /// Returns the position where that key is present, or the position where it can be inserted to
    /// maintain the sort. See [`slice::binary_search`] for more details.
    ///
    /// Computes in **O(log(n))** time, which is notably less scalable than looking the key up
    /// using [`get_index_of`][IndexMap::get_index_of], but this can also position missing keys.
    pub fn binary_search_keys(&self, x: &K) -> Result<usize, usize>
    where
        K: Ord,
    {
        self.as_slice().binary_search_keys(x)
    }

    /// Search over a sorted map with a comparator function.
    ///
    /// Returns the position where that value is present, or the position where it can be inserted
    /// to maintain the sort. See [`slice::binary_search_by`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[inline]
    pub fn binary_search_by<'a, F>(&'a self, f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a K, &'a V) -> Ordering,
    {
        self.as_slice().binary_search_by(f)
    }

    /// Search over a sorted map with an extraction function.
    ///
    /// Returns the position where that value is present, or the position where it can be inserted
    /// to maintain the sort. See [`slice::binary_search_by_key`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[inline]
    pub fn binary_search_by_key<'a, B, F>(&'a self, b: &B, f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a K, &'a V) -> B,
        B: Ord,
    {
        self.as_slice().binary_search_by_key(b, f)
    }

    /// Checks if the keys of this map are sorted.
    #[inline]
    pub fn is_sorted(&self) -> bool
    where
        K: PartialOrd,
    {
        self.as_slice().is_sorted()
    }

    /// Checks if this map is sorted using the given comparator function.
    #[inline]
    pub fn is_sorted_by<'a, F>(&'a self, cmp: F) -> bool
    where
        F: FnMut(&'a K, &'a V, &'a K, &'a V) -> bool,
    {
        self.as_slice().is_sorted_by(cmp)
    }

    /// Checks if this map is sorted using the given sort-key function.
    #[inline]
    pub fn is_sorted_by_key<'a, F, T>(&'a self, sort_key: F) -> bool
    where
        F: FnMut(&'a K, &'a V) -> T,
        T: PartialOrd,
    {
        self.as_slice().is_sorted_by_key(sort_key)
    }

    /// Returns the index of the partition point of a sorted map according to the given predicate
    /// (the index of the first element of the second partition).
    ///
    /// See [`slice::partition_point`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[must_use]
    pub fn partition_point<P>(&self, pred: P) -> usize
    where
        P: FnMut(&K, &V) -> bool,
    {
        self.as_slice().partition_point(pred)
    }

    /// Reverses the order of the map's key-value pairs in place.
    ///
    /// Computes in **O(n)** time and **O(1)** space.
    pub fn reverse(&mut self) {
        self.core.reverse()
    }

    /// Returns a slice of all the key-value pairs in the map.
    ///
    /// Computes in **O(1)** time.
    pub fn as_slice(&self) -> &Slice<K, V> {
        Slice::from_slice(self.as_entries())
    }

    /// Returns a mutable slice of all the key-value pairs in the map.
    ///
    /// Computes in **O(1)** time.
    pub fn as_mut_slice(&mut self) -> &mut Slice<K, V> {
        Slice::from_mut_slice(self.as_entries_mut())
    }

    /// Converts into a boxed slice of all the key-value pairs in the map.
    ///
    /// Note that this will drop the inner hash table and any excess capacity.
    pub fn into_boxed_slice(self) -> Box<Slice<K, V>> {
        Slice::from_boxed(self.into_entries().into_boxed_slice())
    }

    /// Get a key-value pair by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    pub fn get_index(&self, index: usize) -> Option<(&K, &V)> {
        self.as_entries().get(index).map(Bucket::refs)
    }

    /// Get a key-value pair by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    pub fn get_index_mut(&mut self, index: usize) -> Option<(&K, &mut V)> {
        self.as_entries_mut().get_mut(index).map(Bucket::ref_mut)
    }

    /// Get an entry in the map by index for in-place manipulation.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    pub fn get_index_entry(&mut self, index: usize) -> Option<IndexedEntry<'_, K, V>> {
        if index >= self.len() {
            return None;
        }
        Some(IndexedEntry::new(&mut self.core, index))
    }

    /// Get an array of `N` key-value pairs by `N` indices
    ///
    /// Valid indices are *0 <= index < self.len()* and each index needs to be unique.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut map = indexmap::IndexMap::from([(1, 'a'), (3, 'b'), (2, 'c')]);
    /// assert_eq!(map.get_disjoint_indices_mut([2, 0]), Ok([(&2, &mut 'c'), (&1, &mut 'a')]));
    /// ```
    pub fn get_disjoint_indices_mut<const N: usize>(
        &mut self,
        indices: [usize; N],
    ) -> Result<[(&K, &mut V); N], GetDisjointMutError> {
        self.as_mut_slice().get_disjoint_mut(indices)
    }

    /// Returns a slice of key-value pairs in the given range of indices.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    pub fn get_range<R: RangeBounds<usize>>(&self, range: R) -> Option<&Slice<K, V>> {
        let entries = self.as_entries();
        let range = try_simplify_range(range, entries.len())?;
        entries.get(range).map(Slice::from_slice)
    }

    /// Returns a mutable slice of key-value pairs in the given range of indices.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    pub fn get_range_mut<R: RangeBounds<usize>>(&mut self, range: R) -> Option<&mut Slice<K, V>> {
        let entries = self.as_entries_mut();
        let range = try_simplify_range(range, entries.len())?;
        entries.get_mut(range).map(Slice::from_mut_slice)
    }

    /// Get the first key-value pair
    ///
    /// Computes in **O(1)** time.
    #[doc(alias = "first_key_value")] // like `BTreeMap`
    pub fn first(&self) -> Option<(&K, &V)> {
        self.as_entries().first().map(Bucket::refs)
    }

    /// Get the first key-value pair, with mutable access to the value
    ///
    /// Computes in **O(1)** time.
    pub fn first_mut(&mut self) -> Option<(&K, &mut V)> {
        self.as_entries_mut().first_mut().map(Bucket::ref_mut)
    }

    /// Get the first entry in the map for in-place manipulation.
    ///
    /// Computes in **O(1)** time.
    pub fn first_entry(&mut self) -> Option<IndexedEntry<'_, K, V>> {
        self.get_index_entry(0)
    }

    /// Get the last key-value pair
    ///
    /// Computes in **O(1)** time.
    #[doc(alias = "last_key_value")] // like `BTreeMap`
    pub fn last(&self) -> Option<(&K, &V)> {
        self.as_entries().last().map(Bucket::refs)
    }

    /// Get the last key-value pair, with mutable access to the value
    ///
    /// Computes in **O(1)** time.
    pub fn last_mut(&mut self) -> Option<(&K, &mut V)> {
        self.as_entries_mut().last_mut().map(Bucket::ref_mut)
    }

    /// Get the last entry in the map for in-place manipulation.
    ///
    /// Computes in **O(1)** time.
    pub fn last_entry(&mut self) -> Option<IndexedEntry<'_, K, V>> {
        self.get_index_entry(self.len().checked_sub(1)?)
    }

    /// Remove the key-value pair by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Like [`Vec::swap_remove`], the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove_index(&mut self, index: usize) -> Option<(K, V)> {
        self.core.swap_remove_index(index)
    }

    /// Remove the key-value pair by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Like [`Vec::remove`], the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove_index(&mut self, index: usize) -> Option<(K, V)> {
        self.core.shift_remove_index(index)
    }

    /// Moves the position of a key-value pair from one index to another
    /// by shifting all other pairs in-between.
    ///
    /// * If `from < to`, the other pairs will shift down while the targeted pair moves up.
    /// * If `from > to`, the other pairs will shift up while the targeted pair moves down.
    ///
    /// ***Panics*** if `from` or `to` are out of bounds.
    ///
    /// Computes in **O(n)** time (average).
    #[track_caller]
    pub fn move_index(&mut self, from: usize, to: usize) {
        self.core.move_index(from, to)
    }

    /// Swaps the position of two key-value pairs in the map.
    ///
    /// ***Panics*** if `a` or `b` are out of bounds.
    ///
    /// Computes in **O(1)** time (average).
    #[track_caller]
    pub fn swap_indices(&mut self, a: usize, b: usize) {
        self.core.swap_indices(a, b)
    }
}

/// Access [`IndexMap`] values corresponding to a key.
///
/// # Examples
///
/// ```
/// use indexmap::IndexMap;
///
/// let mut map = IndexMap::new();
/// for word in "Lorem ipsum dolor sit amet".split_whitespace() {
///     map.insert(word.to_lowercase(), word.to_uppercase());
/// }
/// assert_eq!(map["lorem"], "LOREM");
/// assert_eq!(map["ipsum"], "IPSUM");
/// ```
///
/// ```should_panic
/// use indexmap::IndexMap;
///
/// let mut map = IndexMap::new();
/// map.insert("foo", 1);
/// println!("{:?}", map["bar"]); // panics!
/// ```
impl<K, V, Q: ?Sized, S> Index<&Q> for IndexMap<K, V, S>
where
    Q: Hash + Equivalent<K>,
    S: BuildHasher,
{
    type Output = V;

    /// Returns a reference to the value corresponding to the supplied `key`.
    ///
    /// ***Panics*** if `key` is not present in the map.
    fn index(&self, key: &Q) -> &V {
        self.get(key).expect("no entry found for key")
    }
}

/// Access [`IndexMap`] values corresponding to a key.
///
/// Mutable indexing allows changing / updating values of key-value
/// pairs that are already present.
///
/// You can **not** insert new pairs with index syntax, use `.insert()`.
///
/// # Examples
///
/// ```
/// use indexmap::IndexMap;
///
/// let mut map = IndexMap::new();
/// for word in "Lorem ipsum dolor sit amet".split_whitespace() {
///     map.insert(word.to_lowercase(), word.to_string());
/// }
/// let lorem = &mut map["lorem"];
/// assert_eq!(lorem, "Lorem");
/// lorem.retain(char::is_lowercase);
/// assert_eq!(map["lorem"], "orem");
/// ```
///
/// ```should_panic
/// use indexmap::IndexMap;
///
/// let mut map = IndexMap::new();
/// map.insert("foo", 1);
/// map["bar"] = 1; // panics!
/// ```
impl<K, V, Q: ?Sized, S> IndexMut<&Q> for IndexMap<K, V, S>
where
    Q: Hash + Equivalent<K>,
    S: BuildHasher,
{
    /// Returns a mutable reference to the value corresponding to the supplied `key`.
    ///
    /// ***Panics*** if `key` is not present in the map.
    fn index_mut(&mut self, key: &Q) -> &mut V {
        self.get_mut(key).expect("no entry found for key")
    }
}

/// Access [`IndexMap`] values at indexed positions.
///
/// See [`Index<usize> for Keys`][keys] to access a map's keys instead.
///
/// [keys]: Keys#impl-Index<usize>-for-Keys<'a,+K,+V>
///
/// # Examples
///
/// ```
/// use indexmap::IndexMap;
///
/// let mut map = IndexMap::new();
/// for word in "Lorem ipsum dolor sit amet".split_whitespace() {
///     map.insert(word.to_lowercase(), word.to_uppercase());
/// }
/// assert_eq!(map[0], "LOREM");
/// assert_eq!(map[1], "IPSUM");
/// map.reverse();
/// assert_eq!(map[0], "AMET");
/// assert_eq!(map[1], "SIT");
/// map.sort_keys();
/// assert_eq!(map[0], "AMET");
/// assert_eq!(map[1], "DOLOR");
/// ```
///
/// ```should_panic
/// use indexmap::IndexMap;
///
/// let mut map = IndexMap::new();
/// map.insert("foo", 1);
/// println!("{:?}", map[10]); // panics!
/// ```
impl<K, V, S> Index<usize> for IndexMap<K, V, S> {
    type Output = V;

    /// Returns a reference to the value at the supplied `index`.
    ///
    /// ***Panics*** if `index` is out of bounds.
    fn index(&self, index: usize) -> &V {
        if let Some((_, value)) = self.get_index(index) {
            value
        } else {
            panic!(
                "index out of bounds: the len is {len} but the index is {index}",
                len = self.len()
            );
        }
    }
}

/// Access [`IndexMap`] values at indexed positions.
///
/// Mutable indexing allows changing / updating indexed values
/// that are already present.
///
/// You can **not** insert new values with index syntax -- use [`.insert()`][IndexMap::insert].
///
/// # Examples
///
/// ```
/// use indexmap::IndexMap;
///
/// let mut map = IndexMap::new();
/// for word in "Lorem ipsum dolor sit amet".split_whitespace() {
///     map.insert(word.to_lowercase(), word.to_string());
/// }
/// let lorem = &mut map[0];
/// assert_eq!(lorem, "Lorem");
/// lorem.retain(char::is_lowercase);
/// assert_eq!(map["lorem"], "orem");
/// ```
///
/// ```should_panic
/// use indexmap::IndexMap;
///
/// let mut map = IndexMap::new();
/// map.insert("foo", 1);
/// map[10] = 1; // panics!
/// ```
impl<K, V, S> IndexMut<usize> for IndexMap<K, V, S> {
    /// Returns a mutable reference to the value at the supplied `index`.
    ///
    /// ***Panics*** if `index` is out of bounds.
    fn index_mut(&mut self, index: usize) -> &mut V {
        let len: usize = self.len();

        if let Some((_, value)) = self.get_index_mut(index) {
            value
        } else {
            panic!("index out of bounds: the len is {len} but the index is {index}");
        }
    }
}

impl<K, V, S> FromIterator<(K, V)> for IndexMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher + Default,
{
    /// Create an `IndexMap` from the sequence of key-value pairs in the
    /// iterable.
    ///
    /// `from_iter` uses the same logic as `extend`. See
    /// [`extend`][IndexMap::extend] for more details.
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iterable: I) -> Self {
        let iter = iterable.into_iter();
        let (low, _) = iter.size_hint();
        let mut map = Self::with_capacity_and_hasher(low, <_>::default());
        map.extend(iter);
        map
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<K, V, const N: usize> From<[(K, V); N]> for IndexMap<K, V, RandomState>
where
    K: Hash + Eq,
{
    /// # Examples
    ///
    /// ```
    /// use indexmap::IndexMap;
    ///
    /// let map1 = IndexMap::from([(1, 2), (3, 4)]);
    /// let map2: IndexMap<_, _> = [(1, 2), (3, 4)].into();
    /// assert_eq!(map1, map2);
    /// ```
    fn from(arr: [(K, V); N]) -> Self {
        Self::from_iter(arr)
    }
}

impl<K, V, S> Extend<(K, V)> for IndexMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    /// Extend the map with all key-value pairs in the iterable.
    ///
    /// This is equivalent to calling [`insert`][IndexMap::insert] for each of
    /// them in order, which means that for keys that already existed
    /// in the map, their value is updated but it keeps the existing order.
    ///
    /// New keys are inserted in the order they appear in the sequence. If
    /// equivalents of a key occur more than once, the last corresponding value
    /// prevails.
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iterable: I) {
        // (Note: this is a copy of `std`/`hashbrown`'s reservation logic.)
        // Keys may be already present or show multiple times in the iterator.
        // Reserve the entire hint lower bound if the map is empty.
        // Otherwise reserve half the hint (rounded up), so the map
        // will only resize twice in the worst case.
        let iter = iterable.into_iter();
        let (lower_len, _) = iter.size_hint();
        let reserve = if self.is_empty() {
            lower_len
        } else {
            lower_len.div_ceil(2)
        };
        self.reserve(reserve);
        iter.for_each(move |(k, v)| {
            self.insert(k, v);
        });
    }
}

impl<'a, K, V, S> Extend<(&'a K, &'a V)> for IndexMap<K, V, S>
where
    K: Hash + Eq + Copy,
    V: Copy,
    S: BuildHasher,
{
    /// Extend the map with all key-value pairs in the iterable.
    ///
    /// See the first extend method for more details.
    fn extend<I: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iterable: I) {
        self.extend(iterable.into_iter().map(|(&key, &value)| (key, value)));
    }
}

impl<K, V, S> Default for IndexMap<K, V, S>
where
    S: Default,
{
    /// Return an empty [`IndexMap`]
    fn default() -> Self {
        Self::with_capacity_and_hasher(0, S::default())
    }
}

impl<K, V1, S1, V2, S2> PartialEq<IndexMap<K, V2, S2>> for IndexMap<K, V1, S1>
where
    K: Hash + Eq,
    V1: PartialEq<V2>,
    S1: BuildHasher,
    S2: BuildHasher,
{
    fn eq(&self, other: &IndexMap<K, V2, S2>) -> bool {
        if self.len() != other.len() {
            return false;
        }

        self.iter()
            .all(|(key, value)| other.get(key).map_or(false, |v| *value == *v))
    }
}

impl<K, V, S> Eq for IndexMap<K, V, S>
where
    K: Eq + Hash,
    V: Eq,
    S: BuildHasher,
{
}
