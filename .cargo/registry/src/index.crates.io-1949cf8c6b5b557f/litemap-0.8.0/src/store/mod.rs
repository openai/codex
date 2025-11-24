// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Traits for pluggable LiteMap backends.
//!
//! By default, LiteMap is backed by a `Vec`. However, in some environments, it may be desirable
//! to use a different data store while still using LiteMap to manage proper ordering of items.
//!
//! The general guidelines for a performant data store are:
//!
//! 1. Must support efficient random access for binary search
//! 2. Should support efficient append operations for deserialization
//!
//! To plug a custom data store into LiteMap, implement:
//!
//! - [`Store`] for most of the methods
//! - [`StoreIterable`] for methods that return iterators
//! - [`StoreFromIterator`] to enable `FromIterator` for LiteMap
//!
//! To test your implementation, enable the `"testing"` Cargo feature and use [`check_store()`].
//!
//! [`check_store()`]: crate::testing::check_store

mod slice_impl;
#[cfg(feature = "alloc")]
mod vec_impl;

use core::cmp::Ordering;
use core::iter::DoubleEndedIterator;
use core::iter::FromIterator;
use core::iter::Iterator;
use core::ops::Range;

/// Trait to enable const construction of empty store.
pub trait StoreConstEmpty<K: ?Sized, V: ?Sized> {
    /// An empty store
    const EMPTY: Self;
}

/// Trait to enable pluggable backends for LiteMap.
///
/// Some methods have default implementations provided for convenience; however, it is generally
/// better to implement all methods that your data store supports.
pub trait Store<K: ?Sized, V: ?Sized>: Sized {
    /// Returns the number of elements in the store.
    fn lm_len(&self) -> usize;

    /// Returns whether the store is empty (contains 0 elements).
    fn lm_is_empty(&self) -> bool {
        self.lm_len() == 0
    }

    /// Gets a key/value pair at the specified index.
    fn lm_get(&self, index: usize) -> Option<(&K, &V)>;

    /// Gets the last element in the store, or `None` if the store is empty.
    fn lm_last(&self) -> Option<(&K, &V)> {
        let len = self.lm_len();
        if len == 0 {
            None
        } else {
            self.lm_get(len - 1)
        }
    }

    /// Searches the store for a particular element with a comparator function.
    ///
    /// See the binary search implementation on `slice` for more information.
    fn lm_binary_search_by<F>(&self, cmp: F) -> Result<usize, usize>
    where
        F: FnMut(&K) -> Ordering;
}

pub trait StoreFromIterable<K, V>: Store<K, V> {
    /// Create a sorted store from `iter`.
    fn lm_sort_from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self;
}

pub trait StoreSlice<K: ?Sized, V: ?Sized>: Store<K, V> {
    type Slice: ?Sized;

    fn lm_get_range(&self, range: Range<usize>) -> Option<&Self::Slice>;
}

pub trait StoreMut<K, V>: Store<K, V> {
    /// Creates a new store with the specified capacity hint.
    ///
    /// Implementations may ignore the argument if they do not support pre-allocating capacity.
    fn lm_with_capacity(capacity: usize) -> Self;

    /// Reserves additional capacity in the store.
    ///
    /// Implementations may ignore the argument if they do not support pre-allocating capacity.
    fn lm_reserve(&mut self, additional: usize);

    /// Gets a key/value pair at the specified index, with a mutable value.
    fn lm_get_mut(&mut self, index: usize) -> Option<(&K, &mut V)>;

    /// Pushes one additional item onto the store.
    fn lm_push(&mut self, key: K, value: V);

    /// Inserts an item at a specific index in the store.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than the length.
    fn lm_insert(&mut self, index: usize, key: K, value: V);

    /// Removes an item at a specific index in the store.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than the length.
    fn lm_remove(&mut self, index: usize) -> (K, V);

    /// Removes all items from the store.
    fn lm_clear(&mut self);
}

pub trait StoreBulkMut<K, V>: StoreMut<K, V> {
    /// Retains items satisfying a predicate in this store.
    fn lm_retain<F>(&mut self, predicate: F)
    where
        F: FnMut(&K, &V) -> bool;

    /// Extends this store with items from an iterator.
    fn lm_extend<I>(&mut self, other: I)
    where
        I: IntoIterator<Item = (K, V)>;
}

/// Iterator methods for the LiteMap store.
pub trait StoreIterable<'a, K: 'a + ?Sized, V: 'a + ?Sized>: Store<K, V> {
    type KeyValueIter: Iterator<Item = (&'a K, &'a V)> + DoubleEndedIterator + 'a;

    /// Returns an iterator over key/value pairs.
    fn lm_iter(&'a self) -> Self::KeyValueIter;
}

pub trait StoreIterableMut<'a, K: 'a, V: 'a>: StoreMut<K, V> + StoreIterable<'a, K, V> {
    type KeyValueIterMut: Iterator<Item = (&'a K, &'a mut V)> + DoubleEndedIterator + 'a;

    /// Returns an iterator over key/value pairs, with a mutable value.
    fn lm_iter_mut(&'a mut self) -> Self::KeyValueIterMut;
}

pub trait StoreIntoIterator<K, V>: StoreMut<K, V> {
    type KeyValueIntoIter: Iterator<Item = (K, V)>;

    /// Returns an iterator that moves every item from this store.
    fn lm_into_iter(self) -> Self::KeyValueIntoIter;

    /// Adds items from another store to the end of this store.
    fn lm_extend_end(&mut self, other: Self)
    where
        Self: Sized,
    {
        for item in other.lm_into_iter() {
            self.lm_push(item.0, item.1);
        }
    }

    /// Adds items from another store to the beginning of this store.
    fn lm_extend_start(&mut self, other: Self)
    where
        Self: Sized,
    {
        for (i, item) in other.lm_into_iter().enumerate() {
            self.lm_insert(i, item.0, item.1);
        }
    }
}

/// A store that can be built from a tuple iterator.
pub trait StoreFromIterator<K, V>: FromIterator<(K, V)> {}
