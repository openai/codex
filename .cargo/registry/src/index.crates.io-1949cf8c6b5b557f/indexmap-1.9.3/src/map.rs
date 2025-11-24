//! `IndexMap` is a hash table where the iteration order of the key-value
//! pairs is independent of the hash values of the keys.

mod core;

pub use crate::mutable_keys::MutableKeys;

#[cfg(feature = "rayon")]
pub use crate::rayon::map as rayon;

use crate::vec::{self, Vec};
use ::core::cmp::Ordering;
use ::core::fmt;
use ::core::hash::{BuildHasher, Hash, Hasher};
use ::core::iter::FusedIterator;
use ::core::ops::{Index, IndexMut, RangeBounds};
use ::core::slice::{Iter as SliceIter, IterMut as SliceIterMut};

#[cfg(has_std)]
use std::collections::hash_map::RandomState;

use self::core::IndexMapCore;
use crate::equivalent::Equivalent;
use crate::util::third;
use crate::{Bucket, Entries, HashValue};

pub use self::core::{Entry, OccupiedEntry, VacantEntry};

/// A hash table where the iteration order of the key-value pairs is independent
/// of the hash values of the keys.
///
/// The interface is closely compatible with the standard `HashMap`, but also
/// has additional features.
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
/// `.remove()` or `.swap_remove()` methods. Methods such as `.sort_by()` of
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
#[cfg(has_std)]
pub struct IndexMap<K, V, S = RandomState> {
    pub(crate) core: IndexMapCore<K, V>,
    hash_builder: S,
}
#[cfg(not(has_std))]
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

impl<K, V, S> Entries for IndexMap<K, V, S> {
    type Entry = Bucket<K, V>;

    #[inline]
    fn into_entries(self) -> Vec<Self::Entry> {
        self.core.into_entries()
    }

    #[inline]
    fn as_entries(&self) -> &[Self::Entry] {
        self.core.as_entries()
    }

    #[inline]
    fn as_entries_mut(&mut self) -> &mut [Self::Entry] {
        self.core.as_entries_mut()
    }

    fn with_entries<F>(&mut self, f: F)
    where
        F: FnOnce(&mut [Self::Entry]),
    {
        self.core.with_entries(f);
    }
}

impl<K, V, S> fmt::Debug for IndexMap<K, V, S>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(not(feature = "test_debug")) {
            f.debug_map().entries(self.iter()).finish()
        } else {
            // Let the inner `IndexMapCore` print all of its details
            f.debug_struct("IndexMap")
                .field("core", &self.core)
                .finish()
        }
    }
}

#[cfg(has_std)]
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
        Iter {
            iter: self.as_entries().iter(),
        }
    }

    /// Return an iterator over the key-value pairs of the map, in their order
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut {
            iter: self.as_entries_mut().iter_mut(),
        }
    }

    /// Return an iterator over the keys of the map, in their order
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys {
            iter: self.as_entries().iter(),
        }
    }

    /// Return an owning iterator over the keys of the map, in their order
    pub fn into_keys(self) -> IntoKeys<K, V> {
        IntoKeys {
            iter: self.into_entries().into_iter(),
        }
    }

    /// Return an iterator over the values of the map, in their order
    pub fn values(&self) -> Values<'_, K, V> {
        Values {
            iter: self.as_entries().iter(),
        }
    }

    /// Return an iterator over mutable references to the values of the map,
    /// in their order
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        ValuesMut {
            iter: self.as_entries_mut().iter_mut(),
        }
    }

    /// Return an owning iterator over the values of the map, in their order
    pub fn into_values(self) -> IntoValues<K, V> {
        IntoValues {
            iter: self.into_entries().into_iter(),
        }
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
    /// The range may be any type that implements `RangeBounds<usize>`,
    /// including all of the `std::ops::Range*` types, or even a tuple pair of
    /// `Bound` start and end values. To drain the map entirely, use `RangeFull`
    /// like `map.drain(..)`.
    ///
    /// This shifts down all entries following the drained range to fill the
    /// gap, and keeps the allocated memory for reuse.
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the map.
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, K, V>
    where
        R: RangeBounds<usize>,
    {
        Drain {
            iter: self.core.drain(range),
        }
    }

    /// Splits the collection into two at the given index.
    ///
    /// Returns a newly allocated map containing the elements in the range
    /// `[at, len)`. After the call, the original map will be left containing
    /// the elements `[0, at)` with its previous capacity unchanged.
    ///
    /// ***Panics*** if `at > len`.
    pub fn split_off(&mut self, at: usize) -> Self
    where
        S: Clone,
    {
        Self {
            core: self.core.split_off(at),
            hash_builder: self.hash_builder.clone(),
        }
    }
}

impl<K, V, S> IndexMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    /// Reserve capacity for `additional` more key-value pairs.
    ///
    /// Computes in **O(n)** time.
    pub fn reserve(&mut self, additional: usize) {
        self.core.reserve(additional);
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

    fn hash<Q: ?Sized + Hash>(&self, key: &Q) -> HashValue {
        let mut h = self.hash_builder.build_hasher();
        key.hash(&mut h);
        HashValue(h.finish() as usize)
    }

    /// Insert a key-value pair in the map.
    ///
    /// If an equivalent key already exists in the map: the key remains and
    /// retains in its place in the order, its corresponding value is updated
    /// with `value` and the older value is returned inside `Some(_)`.
    ///
    /// If no equivalent key existed in the map: the new key-value pair is
    /// inserted, last in order, and `None` is returned.
    ///
    /// Computes in **O(1)** time (amortized average).
    ///
    /// See also [`entry`](#method.entry) if you you want to insert *or* modify
    /// or if you need to get the index of the corresponding key-value pair.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert_full(key, value).1
    }

    /// Insert a key-value pair in the map, and get their index.
    ///
    /// If an equivalent key already exists in the map: the key remains and
    /// retains in its place in the order, its corresponding value is updated
    /// with `value` and the older value is returned inside `(index, Some(_))`.
    ///
    /// If no equivalent key existed in the map: the new key-value pair is
    /// inserted, last in order, and `(index, None)` is returned.
    ///
    /// Computes in **O(1)** time (amortized average).
    ///
    /// See also [`entry`](#method.entry) if you you want to insert *or* modify
    /// or if you need to get the index of the corresponding key-value pair.
    pub fn insert_full(&mut self, key: K, value: V) -> (usize, Option<V>) {
        let hash = self.hash(&key);
        self.core.insert_full(hash, key, value)
    }

    /// Get the given key’s corresponding entry in the map for insertion and/or
    /// in-place manipulation.
    ///
    /// Computes in **O(1)** time (amortized average).
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        let hash = self.hash(&key);
        self.core.entry(hash, key)
    }

    /// Return `true` if an equivalent to `key` exists in the map.
    ///
    /// Computes in **O(1)** time (average).
    pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
    where
        Q: Hash + Equivalent<K>,
    {
        self.get_index_of(key).is_some()
    }

    /// Return a reference to the value stored for `key`, if it is present,
    /// else `None`.
    ///
    /// Computes in **O(1)** time (average).
    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        Q: Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &self.as_entries()[i];
            Some(&entry.value)
        } else {
            None
        }
    }

    /// Return references to the key-value pair stored for `key`,
    /// if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    pub fn get_key_value<Q: ?Sized>(&self, key: &Q) -> Option<(&K, &V)>
    where
        Q: Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &self.as_entries()[i];
            Some((&entry.key, &entry.value))
        } else {
            None
        }
    }

    /// Return item index, key and value
    pub fn get_full<Q: ?Sized>(&self, key: &Q) -> Option<(usize, &K, &V)>
    where
        Q: Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &self.as_entries()[i];
            Some((i, &entry.key, &entry.value))
        } else {
            None
        }
    }

    /// Return item index, if it exists in the map
    ///
    /// Computes in **O(1)** time (average).
    pub fn get_index_of<Q: ?Sized>(&self, key: &Q) -> Option<usize>
    where
        Q: Hash + Equivalent<K>,
    {
        if self.is_empty() {
            None
        } else {
            let hash = self.hash(key);
            self.core.get_index_of(hash, key)
        }
    }

    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        Q: Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &mut self.as_entries_mut()[i];
            Some(&mut entry.value)
        } else {
            None
        }
    }

    pub fn get_full_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<(usize, &K, &mut V)>
    where
        Q: Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &mut self.as_entries_mut()[i];
            Some((i, &entry.key, &mut entry.value))
        } else {
            None
        }
    }

    pub(crate) fn get_full_mut2_impl<Q: ?Sized>(
        &mut self,
        key: &Q,
    ) -> Option<(usize, &mut K, &mut V)>
    where
        Q: Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &mut self.as_entries_mut()[i];
            Some((i, &mut entry.key, &mut entry.value))
        } else {
            None
        }
    }

    /// Remove the key-value pair equivalent to `key` and return
    /// its value.
    ///
    /// **NOTE:** This is equivalent to `.swap_remove(key)`, if you need to
    /// preserve the order of the keys in the map, use `.shift_remove(key)`
    /// instead.
    ///
    /// Computes in **O(1)** time (average).
    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        Q: Hash + Equivalent<K>,
    {
        self.swap_remove(key)
    }

    /// Remove and return the key-value pair equivalent to `key`.
    ///
    /// **NOTE:** This is equivalent to `.swap_remove_entry(key)`, if you need to
    /// preserve the order of the keys in the map, use `.shift_remove_entry(key)`
    /// instead.
    ///
    /// Computes in **O(1)** time (average).
    pub fn remove_entry<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
    where
        Q: Hash + Equivalent<K>,
    {
        self.swap_remove_entry(key)
    }

    /// Remove the key-value pair equivalent to `key` and return
    /// its value.
    ///
    /// Like `Vec::swap_remove`, the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        Q: Hash + Equivalent<K>,
    {
        self.swap_remove_full(key).map(third)
    }

    /// Remove and return the key-value pair equivalent to `key`.
    ///
    /// Like `Vec::swap_remove`, the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove_entry<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
    where
        Q: Hash + Equivalent<K>,
    {
        match self.swap_remove_full(key) {
            Some((_, key, value)) => Some((key, value)),
            None => None,
        }
    }

    /// Remove the key-value pair equivalent to `key` and return it and
    /// the index it had.
    ///
    /// Like `Vec::swap_remove`, the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove_full<Q: ?Sized>(&mut self, key: &Q) -> Option<(usize, K, V)>
    where
        Q: Hash + Equivalent<K>,
    {
        if self.is_empty() {
            return None;
        }
        let hash = self.hash(key);
        self.core.swap_remove_full(hash, key)
    }

    /// Remove the key-value pair equivalent to `key` and return
    /// its value.
    ///
    /// Like `Vec::remove`, the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        Q: Hash + Equivalent<K>,
    {
        self.shift_remove_full(key).map(third)
    }

    /// Remove and return the key-value pair equivalent to `key`.
    ///
    /// Like `Vec::remove`, the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove_entry<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
    where
        Q: Hash + Equivalent<K>,
    {
        match self.shift_remove_full(key) {
            Some((_, key, value)) => Some((key, value)),
            None => None,
        }
    }

    /// Remove the key-value pair equivalent to `key` and return it and
    /// the index it had.
    ///
    /// Like `Vec::remove`, the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove_full<Q: ?Sized>(&mut self, key: &Q) -> Option<(usize, K, V)>
    where
        Q: Hash + Equivalent<K>,
    {
        if self.is_empty() {
            return None;
        }
        let hash = self.hash(key);
        self.core.shift_remove_full(hash, key)
    }

    /// Remove the last key-value pair
    ///
    /// This preserves the order of the remaining elements.
    ///
    /// Computes in **O(1)** time (average).
    pub fn pop(&mut self) -> Option<(K, V)> {
        self.core.pop()
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

    pub(crate) fn retain_mut<F>(&mut self, keep: F)
    where
        F: FnMut(&mut K, &mut V) -> bool,
    {
        self.core.retain_in_order(keep);
    }

    /// Sort the map’s key-value pairs by the default ordering of the keys.
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

    /// Sort the map’s key-value pairs in place using the comparison
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
        IntoIter {
            iter: entries.into_iter(),
        }
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
        IntoIter {
            iter: entries.into_iter(),
        }
    }

    /// Reverses the order of the map’s key-value pairs in place.
    ///
    /// Computes in **O(n)** time and **O(1)** space.
    pub fn reverse(&mut self) {
        self.core.reverse()
    }
}

impl<K, V, S> IndexMap<K, V, S> {
    /// Get a key-value pair by index
    ///
    /// Valid indices are *0 <= index < self.len()*
    ///
    /// Computes in **O(1)** time.
    pub fn get_index(&self, index: usize) -> Option<(&K, &V)> {
        self.as_entries().get(index).map(Bucket::refs)
    }

    /// Get a key-value pair by index
    ///
    /// Valid indices are *0 <= index < self.len()*
    ///
    /// Computes in **O(1)** time.
    pub fn get_index_mut(&mut self, index: usize) -> Option<(&mut K, &mut V)> {
        self.as_entries_mut().get_mut(index).map(Bucket::muts)
    }

    /// Get the first key-value pair
    ///
    /// Computes in **O(1)** time.
    pub fn first(&self) -> Option<(&K, &V)> {
        self.as_entries().first().map(Bucket::refs)
    }

    /// Get the first key-value pair, with mutable access to the value
    ///
    /// Computes in **O(1)** time.
    pub fn first_mut(&mut self) -> Option<(&K, &mut V)> {
        self.as_entries_mut().first_mut().map(Bucket::ref_mut)
    }

    /// Get the last key-value pair
    ///
    /// Computes in **O(1)** time.
    pub fn last(&self) -> Option<(&K, &V)> {
        self.as_entries().last().map(Bucket::refs)
    }

    /// Get the last key-value pair, with mutable access to the value
    ///
    /// Computes in **O(1)** time.
    pub fn last_mut(&mut self) -> Option<(&K, &mut V)> {
        self.as_entries_mut().last_mut().map(Bucket::ref_mut)
    }

    /// Remove the key-value pair by index
    ///
    /// Valid indices are *0 <= index < self.len()*
    ///
    /// Like `Vec::swap_remove`, the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove_index(&mut self, index: usize) -> Option<(K, V)> {
        self.core.swap_remove_index(index)
    }

    /// Remove the key-value pair by index
    ///
    /// Valid indices are *0 <= index < self.len()*
    ///
    /// Like `Vec::remove`, the pair is removed by shifting all of the
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
    pub fn move_index(&mut self, from: usize, to: usize) {
        self.core.move_index(from, to)
    }

    /// Swaps the position of two key-value pairs in the map.
    ///
    /// ***Panics*** if `a` or `b` are out of bounds.
    pub fn swap_indices(&mut self, a: usize, b: usize) {
        self.core.swap_indices(a, b)
    }
}

/// An iterator over the keys of a `IndexMap`.
///
/// This `struct` is created by the [`keys`] method on [`IndexMap`]. See its
/// documentation for more.
///
/// [`keys`]: struct.IndexMap.html#method.keys
/// [`IndexMap`]: struct.IndexMap.html
pub struct Keys<'a, K, V> {
    iter: SliceIter<'a, Bucket<K, V>>,
}

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    iterator_methods!(Bucket::key_ref);
}

impl<K, V> DoubleEndedIterator for Keys<'_, K, V> {
    double_ended_iterator_methods!(Bucket::key_ref);
}

impl<K, V> ExactSizeIterator for Keys<'_, K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for Keys<'_, K, V> {}

// FIXME(#26925) Remove in favor of `#[derive(Clone)]`
impl<K, V> Clone for Keys<'_, K, V> {
    fn clone(&self) -> Self {
        Keys {
            iter: self.iter.clone(),
        }
    }
}

impl<K: fmt::Debug, V> fmt::Debug for Keys<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// An owning iterator over the keys of a `IndexMap`.
///
/// This `struct` is created by the [`into_keys`] method on [`IndexMap`].
/// See its documentation for more.
///
/// [`IndexMap`]: struct.IndexMap.html
/// [`into_keys`]: struct.IndexMap.html#method.into_keys
pub struct IntoKeys<K, V> {
    iter: vec::IntoIter<Bucket<K, V>>,
}

impl<K, V> Iterator for IntoKeys<K, V> {
    type Item = K;

    iterator_methods!(Bucket::key);
}

impl<K, V> DoubleEndedIterator for IntoKeys<K, V> {
    double_ended_iterator_methods!(Bucket::key);
}

impl<K, V> ExactSizeIterator for IntoKeys<K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for IntoKeys<K, V> {}

impl<K: fmt::Debug, V> fmt::Debug for IntoKeys<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.iter.as_slice().iter().map(Bucket::key_ref);
        f.debug_list().entries(iter).finish()
    }
}

/// An iterator over the values of a `IndexMap`.
///
/// This `struct` is created by the [`values`] method on [`IndexMap`]. See its
/// documentation for more.
///
/// [`values`]: struct.IndexMap.html#method.values
/// [`IndexMap`]: struct.IndexMap.html
pub struct Values<'a, K, V> {
    iter: SliceIter<'a, Bucket<K, V>>,
}

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    iterator_methods!(Bucket::value_ref);
}

impl<K, V> DoubleEndedIterator for Values<'_, K, V> {
    double_ended_iterator_methods!(Bucket::value_ref);
}

impl<K, V> ExactSizeIterator for Values<'_, K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for Values<'_, K, V> {}

// FIXME(#26925) Remove in favor of `#[derive(Clone)]`
impl<K, V> Clone for Values<'_, K, V> {
    fn clone(&self) -> Self {
        Values {
            iter: self.iter.clone(),
        }
    }
}

impl<K, V: fmt::Debug> fmt::Debug for Values<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// A mutable iterator over the values of a `IndexMap`.
///
/// This `struct` is created by the [`values_mut`] method on [`IndexMap`]. See its
/// documentation for more.
///
/// [`values_mut`]: struct.IndexMap.html#method.values_mut
/// [`IndexMap`]: struct.IndexMap.html
pub struct ValuesMut<'a, K, V> {
    iter: SliceIterMut<'a, Bucket<K, V>>,
}

impl<'a, K, V> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;

    iterator_methods!(Bucket::value_mut);
}

impl<K, V> DoubleEndedIterator for ValuesMut<'_, K, V> {
    double_ended_iterator_methods!(Bucket::value_mut);
}

impl<K, V> ExactSizeIterator for ValuesMut<'_, K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for ValuesMut<'_, K, V> {}

impl<K, V: fmt::Debug> fmt::Debug for ValuesMut<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.iter.as_slice().iter().map(Bucket::value_ref);
        f.debug_list().entries(iter).finish()
    }
}

/// An owning iterator over the values of a `IndexMap`.
///
/// This `struct` is created by the [`into_values`] method on [`IndexMap`].
/// See its documentation for more.
///
/// [`IndexMap`]: struct.IndexMap.html
/// [`into_values`]: struct.IndexMap.html#method.into_values
pub struct IntoValues<K, V> {
    iter: vec::IntoIter<Bucket<K, V>>,
}

impl<K, V> Iterator for IntoValues<K, V> {
    type Item = V;

    iterator_methods!(Bucket::value);
}

impl<K, V> DoubleEndedIterator for IntoValues<K, V> {
    double_ended_iterator_methods!(Bucket::value);
}

impl<K, V> ExactSizeIterator for IntoValues<K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for IntoValues<K, V> {}

impl<K, V: fmt::Debug> fmt::Debug for IntoValues<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.iter.as_slice().iter().map(Bucket::value_ref);
        f.debug_list().entries(iter).finish()
    }
}

/// An iterator over the entries of a `IndexMap`.
///
/// This `struct` is created by the [`iter`] method on [`IndexMap`]. See its
/// documentation for more.
///
/// [`iter`]: struct.IndexMap.html#method.iter
/// [`IndexMap`]: struct.IndexMap.html
pub struct Iter<'a, K, V> {
    iter: SliceIter<'a, Bucket<K, V>>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    iterator_methods!(Bucket::refs);
}

impl<K, V> DoubleEndedIterator for Iter<'_, K, V> {
    double_ended_iterator_methods!(Bucket::refs);
}

impl<K, V> ExactSizeIterator for Iter<'_, K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for Iter<'_, K, V> {}

// FIXME(#26925) Remove in favor of `#[derive(Clone)]`
impl<K, V> Clone for Iter<'_, K, V> {
    fn clone(&self) -> Self {
        Iter {
            iter: self.iter.clone(),
        }
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for Iter<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// A mutable iterator over the entries of a `IndexMap`.
///
/// This `struct` is created by the [`iter_mut`] method on [`IndexMap`]. See its
/// documentation for more.
///
/// [`iter_mut`]: struct.IndexMap.html#method.iter_mut
/// [`IndexMap`]: struct.IndexMap.html
pub struct IterMut<'a, K, V> {
    iter: SliceIterMut<'a, Bucket<K, V>>,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    iterator_methods!(Bucket::ref_mut);
}

impl<K, V> DoubleEndedIterator for IterMut<'_, K, V> {
    double_ended_iterator_methods!(Bucket::ref_mut);
}

impl<K, V> ExactSizeIterator for IterMut<'_, K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for IterMut<'_, K, V> {}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for IterMut<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.iter.as_slice().iter().map(Bucket::refs);
        f.debug_list().entries(iter).finish()
    }
}

/// An owning iterator over the entries of a `IndexMap`.
///
/// This `struct` is created by the [`into_iter`] method on [`IndexMap`]
/// (provided by the `IntoIterator` trait). See its documentation for more.
///
/// [`into_iter`]: struct.IndexMap.html#method.into_iter
/// [`IndexMap`]: struct.IndexMap.html
pub struct IntoIter<K, V> {
    iter: vec::IntoIter<Bucket<K, V>>,
}

impl<K, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    iterator_methods!(Bucket::key_value);
}

impl<K, V> DoubleEndedIterator for IntoIter<K, V> {
    double_ended_iterator_methods!(Bucket::key_value);
}

impl<K, V> ExactSizeIterator for IntoIter<K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for IntoIter<K, V> {}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for IntoIter<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.iter.as_slice().iter().map(Bucket::refs);
        f.debug_list().entries(iter).finish()
    }
}

/// A draining iterator over the entries of a `IndexMap`.
///
/// This `struct` is created by the [`drain`] method on [`IndexMap`]. See its
/// documentation for more.
///
/// [`drain`]: struct.IndexMap.html#method.drain
/// [`IndexMap`]: struct.IndexMap.html
pub struct Drain<'a, K, V> {
    pub(crate) iter: vec::Drain<'a, Bucket<K, V>>,
}

impl<K, V> Iterator for Drain<'_, K, V> {
    type Item = (K, V);

    iterator_methods!(Bucket::key_value);
}

impl<K, V> DoubleEndedIterator for Drain<'_, K, V> {
    double_ended_iterator_methods!(Bucket::key_value);
}

impl<K, V> ExactSizeIterator for Drain<'_, K, V> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<K, V> FusedIterator for Drain<'_, K, V> {}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for Drain<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let iter = self.iter.as_slice().iter().map(Bucket::refs);
        f.debug_list().entries(iter).finish()
    }
}

impl<'a, K, V, S> IntoIterator for &'a IndexMap<K, V, S> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V, S> IntoIterator for &'a mut IndexMap<K, V, S> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K, V, S> IntoIterator for IndexMap<K, V, S> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            iter: self.into_entries().into_iter(),
        }
    }
}

/// Access `IndexMap` values corresponding to a key.
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
    K: Hash + Eq,
    S: BuildHasher,
{
    type Output = V;

    /// Returns a reference to the value corresponding to the supplied `key`.
    ///
    /// ***Panics*** if `key` is not present in the map.
    fn index(&self, key: &Q) -> &V {
        self.get(key).expect("IndexMap: key not found")
    }
}

/// Access `IndexMap` values corresponding to a key.
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
    K: Hash + Eq,
    S: BuildHasher,
{
    /// Returns a mutable reference to the value corresponding to the supplied `key`.
    ///
    /// ***Panics*** if `key` is not present in the map.
    fn index_mut(&mut self, key: &Q) -> &mut V {
        self.get_mut(key).expect("IndexMap: key not found")
    }
}

/// Access `IndexMap` values at indexed positions.
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
        self.get_index(index)
            .expect("IndexMap: index out of bounds")
            .1
    }
}

/// Access `IndexMap` values at indexed positions.
///
/// Mutable indexing allows changing / updating indexed values
/// that are already present.
///
/// You can **not** insert new values with index syntax, use `.insert()`.
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
        self.get_index_mut(index)
            .expect("IndexMap: index out of bounds")
            .1
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
    /// [`extend`](#method.extend) for more details.
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iterable: I) -> Self {
        let iter = iterable.into_iter();
        let (low, _) = iter.size_hint();
        let mut map = Self::with_capacity_and_hasher(low, <_>::default());
        map.extend(iter);
        map
    }
}

#[cfg(has_std)]
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
    /// This is equivalent to calling [`insert`](#method.insert) for each of
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
        let reserve = if self.is_empty() {
            iter.size_hint().0
        } else {
            (iter.size_hint().0 + 1) / 2
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
    /// Return an empty `IndexMap`
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::String;

    #[test]
    fn it_works() {
        let mut map = IndexMap::new();
        assert_eq!(map.is_empty(), true);
        map.insert(1, ());
        map.insert(1, ());
        assert_eq!(map.len(), 1);
        assert!(map.get(&1).is_some());
        assert_eq!(map.is_empty(), false);
    }

    #[test]
    fn new() {
        let map = IndexMap::<String, String>::new();
        println!("{:?}", map);
        assert_eq!(map.capacity(), 0);
        assert_eq!(map.len(), 0);
        assert_eq!(map.is_empty(), true);
    }

    #[test]
    fn insert() {
        let insert = [0, 4, 2, 12, 8, 7, 11, 5];
        let not_present = [1, 3, 6, 9, 10];
        let mut map = IndexMap::with_capacity(insert.len());

        for (i, &elt) in insert.iter().enumerate() {
            assert_eq!(map.len(), i);
            map.insert(elt, elt);
            assert_eq!(map.len(), i + 1);
            assert_eq!(map.get(&elt), Some(&elt));
            assert_eq!(map[&elt], elt);
        }
        println!("{:?}", map);

        for &elt in &not_present {
            assert!(map.get(&elt).is_none());
        }
    }

    #[test]
    fn insert_full() {
        let insert = vec![9, 2, 7, 1, 4, 6, 13];
        let present = vec![1, 6, 2];
        let mut map = IndexMap::with_capacity(insert.len());

        for (i, &elt) in insert.iter().enumerate() {
            assert_eq!(map.len(), i);
            let (index, existing) = map.insert_full(elt, elt);
            assert_eq!(existing, None);
            assert_eq!(Some(index), map.get_full(&elt).map(|x| x.0));
            assert_eq!(map.len(), i + 1);
        }

        let len = map.len();
        for &elt in &present {
            let (index, existing) = map.insert_full(elt, elt);
            assert_eq!(existing, Some(elt));
            assert_eq!(Some(index), map.get_full(&elt).map(|x| x.0));
            assert_eq!(map.len(), len);
        }
    }

    #[test]
    fn insert_2() {
        let mut map = IndexMap::with_capacity(16);

        let mut keys = vec![];
        keys.extend(0..16);
        keys.extend(if cfg!(miri) { 32..64 } else { 128..267 });

        for &i in &keys {
            let old_map = map.clone();
            map.insert(i, ());
            for key in old_map.keys() {
                if map.get(key).is_none() {
                    println!("old_map: {:?}", old_map);
                    println!("map: {:?}", map);
                    panic!("did not find {} in map", key);
                }
            }
        }

        for &i in &keys {
            assert!(map.get(&i).is_some(), "did not find {}", i);
        }
    }

    #[test]
    fn insert_order() {
        let insert = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
        let mut map = IndexMap::new();

        for &elt in &insert {
            map.insert(elt, ());
        }

        assert_eq!(map.keys().count(), map.len());
        assert_eq!(map.keys().count(), insert.len());
        for (a, b) in insert.iter().zip(map.keys()) {
            assert_eq!(a, b);
        }
        for (i, k) in (0..insert.len()).zip(map.keys()) {
            assert_eq!(map.get_index(i).unwrap().0, k);
        }
    }

    #[test]
    fn grow() {
        let insert = [0, 4, 2, 12, 8, 7, 11];
        let not_present = [1, 3, 6, 9, 10];
        let mut map = IndexMap::with_capacity(insert.len());

        for (i, &elt) in insert.iter().enumerate() {
            assert_eq!(map.len(), i);
            map.insert(elt, elt);
            assert_eq!(map.len(), i + 1);
            assert_eq!(map.get(&elt), Some(&elt));
            assert_eq!(map[&elt], elt);
        }

        println!("{:?}", map);
        for &elt in &insert {
            map.insert(elt * 10, elt);
        }
        for &elt in &insert {
            map.insert(elt * 100, elt);
        }
        for (i, &elt) in insert.iter().cycle().enumerate().take(100) {
            map.insert(elt * 100 + i as i32, elt);
        }
        println!("{:?}", map);
        for &elt in &not_present {
            assert!(map.get(&elt).is_none());
        }
    }

    #[test]
    fn reserve() {
        let mut map = IndexMap::<usize, usize>::new();
        assert_eq!(map.capacity(), 0);
        map.reserve(100);
        let capacity = map.capacity();
        assert!(capacity >= 100);
        for i in 0..capacity {
            assert_eq!(map.len(), i);
            map.insert(i, i * i);
            assert_eq!(map.len(), i + 1);
            assert_eq!(map.capacity(), capacity);
            assert_eq!(map.get(&i), Some(&(i * i)));
        }
        map.insert(capacity, std::usize::MAX);
        assert_eq!(map.len(), capacity + 1);
        assert!(map.capacity() > capacity);
        assert_eq!(map.get(&capacity), Some(&std::usize::MAX));
    }

    #[test]
    fn shrink_to_fit() {
        let mut map = IndexMap::<usize, usize>::new();
        assert_eq!(map.capacity(), 0);
        for i in 0..100 {
            assert_eq!(map.len(), i);
            map.insert(i, i * i);
            assert_eq!(map.len(), i + 1);
            assert!(map.capacity() >= i + 1);
            assert_eq!(map.get(&i), Some(&(i * i)));
            map.shrink_to_fit();
            assert_eq!(map.len(), i + 1);
            assert_eq!(map.capacity(), i + 1);
            assert_eq!(map.get(&i), Some(&(i * i)));
        }
    }

    #[test]
    fn remove() {
        let insert = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
        let mut map = IndexMap::new();

        for &elt in &insert {
            map.insert(elt, elt);
        }

        assert_eq!(map.keys().count(), map.len());
        assert_eq!(map.keys().count(), insert.len());
        for (a, b) in insert.iter().zip(map.keys()) {
            assert_eq!(a, b);
        }

        let remove_fail = [99, 77];
        let remove = [4, 12, 8, 7];

        for &key in &remove_fail {
            assert!(map.swap_remove_full(&key).is_none());
        }
        println!("{:?}", map);
        for &key in &remove {
            //println!("{:?}", map);
            let index = map.get_full(&key).unwrap().0;
            assert_eq!(map.swap_remove_full(&key), Some((index, key, key)));
        }
        println!("{:?}", map);

        for key in &insert {
            assert_eq!(map.get(key).is_some(), !remove.contains(key));
        }
        assert_eq!(map.len(), insert.len() - remove.len());
        assert_eq!(map.keys().count(), insert.len() - remove.len());
    }

    #[test]
    fn remove_to_empty() {
        let mut map = indexmap! { 0 => 0, 4 => 4, 5 => 5 };
        map.swap_remove(&5).unwrap();
        map.swap_remove(&4).unwrap();
        map.swap_remove(&0).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn swap_remove_index() {
        let insert = [0, 4, 2, 12, 8, 7, 11, 5, 3, 17, 19, 22, 23];
        let mut map = IndexMap::new();

        for &elt in &insert {
            map.insert(elt, elt * 2);
        }

        let mut vector = insert.to_vec();
        let remove_sequence = &[3, 3, 10, 4, 5, 4, 3, 0, 1];

        // check that the same swap remove sequence on vec and map
        // have the same result.
        for &rm in remove_sequence {
            let out_vec = vector.swap_remove(rm);
            let (out_map, _) = map.swap_remove_index(rm).unwrap();
            assert_eq!(out_vec, out_map);
        }
        assert_eq!(vector.len(), map.len());
        for (a, b) in vector.iter().zip(map.keys()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn partial_eq_and_eq() {
        let mut map_a = IndexMap::new();
        map_a.insert(1, "1");
        map_a.insert(2, "2");
        let mut map_b = map_a.clone();
        assert_eq!(map_a, map_b);
        map_b.swap_remove(&1);
        assert_ne!(map_a, map_b);

        let map_c: IndexMap<_, String> = map_b.into_iter().map(|(k, v)| (k, v.into())).collect();
        assert_ne!(map_a, map_c);
        assert_ne!(map_c, map_a);
    }

    #[test]
    fn extend() {
        let mut map = IndexMap::new();
        map.extend(vec![(&1, &2), (&3, &4)]);
        map.extend(vec![(5, 6)]);
        assert_eq!(
            map.into_iter().collect::<Vec<_>>(),
            vec![(1, 2), (3, 4), (5, 6)]
        );
    }

    #[test]
    fn entry() {
        let mut map = IndexMap::new();

        map.insert(1, "1");
        map.insert(2, "2");
        {
            let e = map.entry(3);
            assert_eq!(e.index(), 2);
            let e = e.or_insert("3");
            assert_eq!(e, &"3");
        }

        let e = map.entry(2);
        assert_eq!(e.index(), 1);
        assert_eq!(e.key(), &2);
        match e {
            Entry::Occupied(ref e) => assert_eq!(e.get(), &"2"),
            Entry::Vacant(_) => panic!(),
        }
        assert_eq!(e.or_insert("4"), &"2");
    }

    #[test]
    fn entry_and_modify() {
        let mut map = IndexMap::new();

        map.insert(1, "1");
        map.entry(1).and_modify(|x| *x = "2");
        assert_eq!(Some(&"2"), map.get(&1));

        map.entry(2).and_modify(|x| *x = "doesn't exist");
        assert_eq!(None, map.get(&2));
    }

    #[test]
    fn entry_or_default() {
        let mut map = IndexMap::new();

        #[derive(Debug, PartialEq)]
        enum TestEnum {
            DefaultValue,
            NonDefaultValue,
        }

        impl Default for TestEnum {
            fn default() -> Self {
                TestEnum::DefaultValue
            }
        }

        map.insert(1, TestEnum::NonDefaultValue);
        assert_eq!(&mut TestEnum::NonDefaultValue, map.entry(1).or_default());

        assert_eq!(&mut TestEnum::DefaultValue, map.entry(2).or_default());
    }

    #[test]
    fn occupied_entry_key() {
        // These keys match hash and equality, but their addresses are distinct.
        let (k1, k2) = (&mut 1, &mut 1);
        let k1_ptr = k1 as *const i32;
        let k2_ptr = k2 as *const i32;
        assert_ne!(k1_ptr, k2_ptr);

        let mut map = IndexMap::new();
        map.insert(k1, "value");
        match map.entry(k2) {
            Entry::Occupied(ref e) => {
                // `OccupiedEntry::key` should reference the key in the map,
                // not the key that was used to find the entry.
                let ptr = *e.key() as *const i32;
                assert_eq!(ptr, k1_ptr);
                assert_ne!(ptr, k2_ptr);
            }
            Entry::Vacant(_) => panic!(),
        }
    }

    #[test]
    fn keys() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: IndexMap<_, _> = vec.into_iter().collect();
        let keys: Vec<_> = map.keys().copied().collect();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&1));
        assert!(keys.contains(&2));
        assert!(keys.contains(&3));
    }

    #[test]
    fn into_keys() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: IndexMap<_, _> = vec.into_iter().collect();
        let keys: Vec<i32> = map.into_keys().collect();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&1));
        assert!(keys.contains(&2));
        assert!(keys.contains(&3));
    }

    #[test]
    fn values() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: IndexMap<_, _> = vec.into_iter().collect();
        let values: Vec<_> = map.values().copied().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&'a'));
        assert!(values.contains(&'b'));
        assert!(values.contains(&'c'));
    }

    #[test]
    fn values_mut() {
        let vec = vec![(1, 1), (2, 2), (3, 3)];
        let mut map: IndexMap<_, _> = vec.into_iter().collect();
        for value in map.values_mut() {
            *value *= 2
        }
        let values: Vec<_> = map.values().copied().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&2));
        assert!(values.contains(&4));
        assert!(values.contains(&6));
    }

    #[test]
    fn into_values() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: IndexMap<_, _> = vec.into_iter().collect();
        let values: Vec<char> = map.into_values().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&'a'));
        assert!(values.contains(&'b'));
        assert!(values.contains(&'c'));
    }

    #[test]
    #[cfg(has_std)]
    fn from_array() {
        let map = IndexMap::from([(1, 2), (3, 4)]);
        let mut expected = IndexMap::new();
        expected.insert(1, 2);
        expected.insert(3, 4);

        assert_eq!(map, expected)
    }
}
