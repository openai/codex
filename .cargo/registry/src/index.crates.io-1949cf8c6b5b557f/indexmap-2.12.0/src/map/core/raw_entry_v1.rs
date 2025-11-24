//! Opt-in access to the experimental raw entry API.
//!
//! This module is designed to mimic the raw entry API of [`HashMap`][std::collections::hash_map],
//! matching its unstable state as of Rust 1.75. See the tracking issue
//! [rust#56167](https://github.com/rust-lang/rust/issues/56167) for more details.
//!
//! The trait [`RawEntryApiV1`] and the `_v1` suffix on its methods are meant to insulate this for
//! the future, in case later breaking changes are needed. If the standard library stabilizes its
//! `hash_raw_entry` feature (or some replacement), matching *inherent* methods will be added to
//! `IndexMap` without such an opt-in trait.

use super::{Entries, RefMut};
use crate::{Equivalent, HashValue, IndexMap};
use core::fmt;
use core::hash::{BuildHasher, Hash};
use core::marker::PhantomData;
use core::mem;
use hashbrown::hash_table;

/// Opt-in access to the experimental raw entry API.
///
/// See the [`raw_entry_v1`][self] module documentation for more information.
#[expect(private_bounds)]
pub trait RawEntryApiV1<K, V, S>: Sealed {
    /// Creates a raw immutable entry builder for the [`IndexMap`].
    ///
    /// Raw entries provide the lowest level of control for searching and
    /// manipulating a map. They must be manually initialized with a hash and
    /// then manually searched.
    ///
    /// This is useful for
    /// * Hash memoization
    /// * Using a search key that doesn't work with the [`Equivalent`] trait
    /// * Using custom comparison logic without newtype wrappers
    ///
    /// Unless you are in such a situation, higher-level and more foolproof APIs like
    /// [`get`][IndexMap::get] should be preferred.
    ///
    /// Immutable raw entries have very limited use; you might instead want
    /// [`raw_entry_mut_v1`][Self::raw_entry_mut_v1].
    ///
    /// # Examples
    ///
    /// ```
    /// use core::hash::BuildHasher;
    /// use indexmap::map::{IndexMap, RawEntryApiV1};
    ///
    /// let mut map = IndexMap::new();
    /// map.extend([("a", 100), ("b", 200), ("c", 300)]);
    ///
    /// for k in ["a", "b", "c", "d", "e", "f"] {
    ///     let hash = map.hasher().hash_one(k);
    ///     let i = map.get_index_of(k);
    ///     let v = map.get(k);
    ///     let kv = map.get_key_value(k);
    ///     let ikv = map.get_full(k);
    ///
    ///     println!("Key: {} and value: {:?}", k, v);
    ///
    ///     assert_eq!(map.raw_entry_v1().from_key(k), kv);
    ///     assert_eq!(map.raw_entry_v1().from_hash(hash, |q| *q == k), kv);
    ///     assert_eq!(map.raw_entry_v1().from_key_hashed_nocheck(hash, k), kv);
    ///     assert_eq!(map.raw_entry_v1().from_hash_full(hash, |q| *q == k), ikv);
    ///     assert_eq!(map.raw_entry_v1().index_from_hash(hash, |q| *q == k), i);
    /// }
    /// ```
    fn raw_entry_v1(&self) -> RawEntryBuilder<'_, K, V, S>;

    /// Creates a raw entry builder for the [`IndexMap`].
    ///
    /// Raw entries provide the lowest level of control for searching and
    /// manipulating a map. They must be manually initialized with a hash and
    /// then manually searched. After this, insertions into a vacant entry
    /// still require an owned key to be provided.
    ///
    /// Raw entries are useful for such exotic situations as:
    ///
    /// * Hash memoization
    /// * Deferring the creation of an owned key until it is known to be required
    /// * Using a search key that doesn't work with the [`Equivalent`] trait
    /// * Using custom comparison logic without newtype wrappers
    ///
    /// Because raw entries provide much more low-level control, it's much easier
    /// to put the `IndexMap` into an inconsistent state which, while memory-safe,
    /// will cause the map to produce seemingly random results. Higher-level and more
    /// foolproof APIs like [`entry`][IndexMap::entry] should be preferred when possible.
    ///
    /// Raw entries give mutable access to the keys. This must not be used
    /// to modify how the key would compare or hash, as the map will not re-evaluate
    /// where the key should go, meaning the keys may become "lost" if their
    /// location does not reflect their state. For instance, if you change a key
    /// so that the map now contains keys which compare equal, search may start
    /// acting erratically, with two keys randomly masking each other. Implementations
    /// are free to assume this doesn't happen (within the limits of memory-safety).
    ///
    /// # Examples
    ///
    /// ```
    /// use core::hash::BuildHasher;
    /// use indexmap::map::{IndexMap, RawEntryApiV1};
    /// use indexmap::map::raw_entry_v1::RawEntryMut;
    ///
    /// let mut map = IndexMap::new();
    /// map.extend([("a", 100), ("b", 200), ("c", 300)]);
    ///
    /// // Existing key (insert and update)
    /// match map.raw_entry_mut_v1().from_key("a") {
    ///     RawEntryMut::Vacant(_) => unreachable!(),
    ///     RawEntryMut::Occupied(mut view) => {
    ///         assert_eq!(view.index(), 0);
    ///         assert_eq!(view.get(), &100);
    ///         let v = view.get_mut();
    ///         let new_v = (*v) * 10;
    ///         *v = new_v;
    ///         assert_eq!(view.insert(1111), 1000);
    ///     }
    /// }
    ///
    /// assert_eq!(map["a"], 1111);
    /// assert_eq!(map.len(), 3);
    ///
    /// // Existing key (take)
    /// let hash = map.hasher().hash_one("c");
    /// match map.raw_entry_mut_v1().from_key_hashed_nocheck(hash, "c") {
    ///     RawEntryMut::Vacant(_) => unreachable!(),
    ///     RawEntryMut::Occupied(view) => {
    ///         assert_eq!(view.index(), 2);
    ///         assert_eq!(view.shift_remove_entry(), ("c", 300));
    ///     }
    /// }
    /// assert_eq!(map.raw_entry_v1().from_key("c"), None);
    /// assert_eq!(map.len(), 2);
    ///
    /// // Nonexistent key (insert and update)
    /// let key = "d";
    /// let hash = map.hasher().hash_one(key);
    /// match map.raw_entry_mut_v1().from_hash(hash, |q| *q == key) {
    ///     RawEntryMut::Occupied(_) => unreachable!(),
    ///     RawEntryMut::Vacant(view) => {
    ///         assert_eq!(view.index(), 2);
    ///         let (k, value) = view.insert("d", 4000);
    ///         assert_eq!((*k, *value), ("d", 4000));
    ///         *value = 40000;
    ///     }
    /// }
    /// assert_eq!(map["d"], 40000);
    /// assert_eq!(map.len(), 3);
    ///
    /// match map.raw_entry_mut_v1().from_hash(hash, |q| *q == key) {
    ///     RawEntryMut::Vacant(_) => unreachable!(),
    ///     RawEntryMut::Occupied(view) => {
    ///         assert_eq!(view.index(), 2);
    ///         assert_eq!(view.swap_remove_entry(), ("d", 40000));
    ///     }
    /// }
    /// assert_eq!(map.get("d"), None);
    /// assert_eq!(map.len(), 2);
    /// ```
    fn raw_entry_mut_v1(&mut self) -> RawEntryBuilderMut<'_, K, V, S>;
}

impl<K, V, S> RawEntryApiV1<K, V, S> for IndexMap<K, V, S> {
    fn raw_entry_v1(&self) -> RawEntryBuilder<'_, K, V, S> {
        RawEntryBuilder { map: self }
    }

    fn raw_entry_mut_v1(&mut self) -> RawEntryBuilderMut<'_, K, V, S> {
        RawEntryBuilderMut { map: self }
    }
}

/// A builder for computing where in an [`IndexMap`] a key-value pair would be stored.
///
/// This `struct` is created by the [`IndexMap::raw_entry_v1`] method, provided by the
/// [`RawEntryApiV1`] trait. See its documentation for more.
pub struct RawEntryBuilder<'a, K, V, S> {
    map: &'a IndexMap<K, V, S>,
}

impl<K, V, S> fmt::Debug for RawEntryBuilder<'_, K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawEntryBuilder").finish_non_exhaustive()
    }
}

impl<'a, K, V, S> RawEntryBuilder<'a, K, V, S> {
    /// Access an entry by key.
    pub fn from_key<Q>(self, key: &Q) -> Option<(&'a K, &'a V)>
    where
        S: BuildHasher,
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.map.get_key_value(key)
    }

    /// Access an entry by a key and its hash.
    pub fn from_key_hashed_nocheck<Q>(self, hash: u64, key: &Q) -> Option<(&'a K, &'a V)>
    where
        Q: ?Sized + Equivalent<K>,
    {
        let hash = HashValue(hash as usize);
        let i = self.map.core.get_index_of(hash, key)?;
        self.map.get_index(i)
    }

    /// Access an entry by hash.
    pub fn from_hash<F>(self, hash: u64, is_match: F) -> Option<(&'a K, &'a V)>
    where
        F: FnMut(&K) -> bool,
    {
        let map = self.map;
        let i = self.index_from_hash(hash, is_match)?;
        map.get_index(i)
    }

    /// Access an entry by hash, including its index.
    pub fn from_hash_full<F>(self, hash: u64, is_match: F) -> Option<(usize, &'a K, &'a V)>
    where
        F: FnMut(&K) -> bool,
    {
        let map = self.map;
        let i = self.index_from_hash(hash, is_match)?;
        let (key, value) = map.get_index(i)?;
        Some((i, key, value))
    }

    /// Access the index of an entry by hash.
    pub fn index_from_hash<F>(self, hash: u64, mut is_match: F) -> Option<usize>
    where
        F: FnMut(&K) -> bool,
    {
        let hash = HashValue(hash as usize);
        let entries = &*self.map.core.entries;
        let eq = move |&i: &usize| is_match(&entries[i].key);
        self.map.core.indices.find(hash.get(), eq).copied()
    }
}

/// A builder for computing where in an [`IndexMap`] a key-value pair would be stored.
///
/// This `struct` is created by the [`IndexMap::raw_entry_mut_v1`] method, provided by the
/// [`RawEntryApiV1`] trait. See its documentation for more.
pub struct RawEntryBuilderMut<'a, K, V, S> {
    map: &'a mut IndexMap<K, V, S>,
}

impl<K, V, S> fmt::Debug for RawEntryBuilderMut<'_, K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawEntryBuilderMut").finish_non_exhaustive()
    }
}

impl<'a, K, V, S> RawEntryBuilderMut<'a, K, V, S> {
    /// Access an entry by key.
    pub fn from_key<Q>(self, key: &Q) -> RawEntryMut<'a, K, V, S>
    where
        S: BuildHasher,
        Q: ?Sized + Hash + Equivalent<K>,
    {
        let hash = self.map.hash(key);
        self.from_key_hashed_nocheck(hash.get(), key)
    }

    /// Access an entry by a key and its hash.
    pub fn from_key_hashed_nocheck<Q>(self, hash: u64, key: &Q) -> RawEntryMut<'a, K, V, S>
    where
        Q: ?Sized + Equivalent<K>,
    {
        self.from_hash(hash, |k| Q::equivalent(key, k))
    }

    /// Access an entry by hash.
    pub fn from_hash<F>(self, hash: u64, mut is_match: F) -> RawEntryMut<'a, K, V, S>
    where
        F: FnMut(&K) -> bool,
    {
        let ref_entries = &*self.map.core.entries;
        let eq = move |&i: &usize| is_match(&ref_entries[i].key);
        match self.map.core.indices.find_entry(hash, eq) {
            Ok(index) => RawEntryMut::Occupied(RawOccupiedEntryMut {
                entries: &mut self.map.core.entries,
                index,
                hash_builder: PhantomData,
            }),
            Err(absent) => RawEntryMut::Vacant(RawVacantEntryMut {
                map: RefMut::new(absent.into_table(), &mut self.map.core.entries),
                hash_builder: &self.map.hash_builder,
            }),
        }
    }
}

/// Raw entry for an existing key-value pair or a vacant location to
/// insert one.
pub enum RawEntryMut<'a, K, V, S> {
    /// Existing slot with equivalent key.
    Occupied(RawOccupiedEntryMut<'a, K, V, S>),
    /// Vacant slot (no equivalent key in the map).
    Vacant(RawVacantEntryMut<'a, K, V, S>),
}

impl<K: fmt::Debug, V: fmt::Debug, S> fmt::Debug for RawEntryMut<'_, K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut tuple = f.debug_tuple("RawEntryMut");
        match self {
            Self::Vacant(v) => tuple.field(v),
            Self::Occupied(o) => tuple.field(o),
        };
        tuple.finish()
    }
}

impl<'a, K, V, S> RawEntryMut<'a, K, V, S> {
    /// Return the index where the key-value pair exists or may be inserted.
    #[inline]
    pub fn index(&self) -> usize {
        match self {
            Self::Occupied(entry) => entry.index(),
            Self::Vacant(entry) => entry.index(),
        }
    }

    /// Inserts the given default key and value in the entry if it is vacant and returns mutable
    /// references to them. Otherwise mutable references to an already existent pair are returned.
    pub fn or_insert(self, default_key: K, default_value: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
        S: BuildHasher,
    {
        match self {
            Self::Occupied(entry) => entry.into_key_value_mut(),
            Self::Vacant(entry) => entry.insert(default_key, default_value),
        }
    }

    /// Inserts the result of the `call` function in the entry if it is vacant and returns mutable
    /// references to them. Otherwise mutable references to an already existent pair are returned.
    pub fn or_insert_with<F>(self, call: F) -> (&'a mut K, &'a mut V)
    where
        F: FnOnce() -> (K, V),
        K: Hash,
        S: BuildHasher,
    {
        match self {
            Self::Occupied(entry) => entry.into_key_value_mut(),
            Self::Vacant(entry) => {
                let (key, value) = call();
                entry.insert(key, value)
            }
        }
    }

    /// Modifies the entry if it is occupied.
    pub fn and_modify<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut K, &mut V),
    {
        if let Self::Occupied(entry) = &mut self {
            let (k, v) = entry.get_key_value_mut();
            f(k, v);
        }
        self
    }
}

/// A raw view into an occupied entry in an [`IndexMap`].
/// It is part of the [`RawEntryMut`] enum.
pub struct RawOccupiedEntryMut<'a, K, V, S> {
    entries: &'a mut Entries<K, V>,
    index: hash_table::OccupiedEntry<'a, usize>,
    hash_builder: PhantomData<&'a S>,
}

impl<K: fmt::Debug, V: fmt::Debug, S> fmt::Debug for RawOccupiedEntryMut<'_, K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawOccupiedEntryMut")
            .field("key", self.key())
            .field("value", self.get())
            .finish_non_exhaustive()
    }
}

impl<'a, K, V, S> RawOccupiedEntryMut<'a, K, V, S> {
    /// Return the index of the key-value pair
    #[inline]
    pub fn index(&self) -> usize {
        *self.index.get()
    }

    #[inline]
    fn into_ref_mut(self) -> RefMut<'a, K, V> {
        RefMut::new(self.index.into_table(), self.entries)
    }

    /// Gets a reference to the entry's key in the map.
    ///
    /// Note that this is not the key that was used to find the entry. There may be an observable
    /// difference if the key type has any distinguishing features outside of `Hash` and `Eq`, like
    /// extra fields or the memory address of an allocation.
    pub fn key(&self) -> &K {
        &self.entries[self.index()].key
    }

    /// Gets a mutable reference to the entry's key in the map.
    ///
    /// Note that this is not the key that was used to find the entry. There may be an observable
    /// difference if the key type has any distinguishing features outside of `Hash` and `Eq`, like
    /// extra fields or the memory address of an allocation.
    pub fn key_mut(&mut self) -> &mut K {
        let index = self.index();
        &mut self.entries[index].key
    }

    /// Converts into a mutable reference to the entry's key in the map,
    /// with a lifetime bound to the map itself.
    ///
    /// Note that this is not the key that was used to find the entry. There may be an observable
    /// difference if the key type has any distinguishing features outside of `Hash` and `Eq`, like
    /// extra fields or the memory address of an allocation.
    pub fn into_key(self) -> &'a mut K {
        let index = self.index();
        &mut self.entries[index].key
    }

    /// Gets a reference to the entry's value in the map.
    pub fn get(&self) -> &V {
        &self.entries[self.index()].value
    }

    /// Gets a mutable reference to the entry's value in the map.
    ///
    /// If you need a reference which may outlive the destruction of the
    /// [`RawEntryMut`] value, see [`into_mut`][Self::into_mut].
    pub fn get_mut(&mut self) -> &mut V {
        let index = self.index();
        &mut self.entries[index].value
    }

    /// Converts into a mutable reference to the entry's value in the map,
    /// with a lifetime bound to the map itself.
    pub fn into_mut(self) -> &'a mut V {
        let index = self.index();
        &mut self.entries[index].value
    }

    /// Gets a reference to the entry's key and value in the map.
    pub fn get_key_value(&self) -> (&K, &V) {
        self.entries[self.index()].refs()
    }

    /// Gets a reference to the entry's key and value in the map.
    pub fn get_key_value_mut(&mut self) -> (&mut K, &mut V) {
        let index = self.index();
        self.entries[index].muts()
    }

    /// Converts into a mutable reference to the entry's key and value in the map,
    /// with a lifetime bound to the map itself.
    pub fn into_key_value_mut(self) -> (&'a mut K, &'a mut V) {
        let index = self.index();
        self.entries[index].muts()
    }

    /// Sets the value of the entry, and returns the entry's old value.
    pub fn insert(&mut self, value: V) -> V {
        mem::replace(self.get_mut(), value)
    }

    /// Sets the key of the entry, and returns the entry's old key.
    pub fn insert_key(&mut self, key: K) -> K {
        mem::replace(self.key_mut(), key)
    }

    /// Remove the key, value pair stored in the map for this entry, and return the value.
    ///
    /// **NOTE:** This is equivalent to [`.swap_remove()`][Self::swap_remove], replacing this
    /// entry's position with the last element, and it is deprecated in favor of calling that
    /// explicitly. If you need to preserve the relative order of the keys in the map, use
    /// [`.shift_remove()`][Self::shift_remove] instead.
    #[deprecated(note = "`remove` disrupts the map order -- \
        use `swap_remove` or `shift_remove` for explicit behavior.")]
    pub fn remove(self) -> V {
        self.swap_remove()
    }

    /// Remove the key, value pair stored in the map for this entry, and return the value.
    ///
    /// Like [`Vec::swap_remove`][alloc::vec::Vec::swap_remove], the pair is removed by swapping it
    /// with the last element of the map and popping it off.
    /// **This perturbs the position of what used to be the last element!**
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove(self) -> V {
        self.swap_remove_entry().1
    }

    /// Remove the key, value pair stored in the map for this entry, and return the value.
    ///
    /// Like [`Vec::remove`][alloc::vec::Vec::remove], the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove(self) -> V {
        self.shift_remove_entry().1
    }

    /// Remove and return the key, value pair stored in the map for this entry
    ///
    /// **NOTE:** This is equivalent to [`.swap_remove_entry()`][Self::swap_remove_entry],
    /// replacing this entry's position with the last element, and it is deprecated in favor of
    /// calling that explicitly. If you need to preserve the relative order of the keys in the map,
    /// use [`.shift_remove_entry()`][Self::shift_remove_entry] instead.
    #[deprecated(note = "`remove_entry` disrupts the map order -- \
        use `swap_remove_entry` or `shift_remove_entry` for explicit behavior.")]
    pub fn remove_entry(self) -> (K, V) {
        self.swap_remove_entry()
    }

    /// Remove and return the key, value pair stored in the map for this entry
    ///
    /// Like [`Vec::swap_remove`][alloc::vec::Vec::swap_remove], the pair is removed by swapping it
    /// with the last element of the map and popping it off.
    /// **This perturbs the position of what used to be the last element!**
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove_entry(self) -> (K, V) {
        let (index, entry) = self.index.remove();
        RefMut::new(entry.into_table(), self.entries).swap_remove_finish(index)
    }

    /// Remove and return the key, value pair stored in the map for this entry
    ///
    /// Like [`Vec::remove`][alloc::vec::Vec::remove], the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove_entry(self) -> (K, V) {
        let (index, entry) = self.index.remove();
        RefMut::new(entry.into_table(), self.entries).shift_remove_finish(index)
    }

    /// Moves the position of the entry to a new index
    /// by shifting all other entries in-between.
    ///
    /// This is equivalent to [`IndexMap::move_index`]
    /// coming `from` the current [`.index()`][Self::index].
    ///
    /// * If `self.index() < to`, the other pairs will shift down while the targeted pair moves up.
    /// * If `self.index() > to`, the other pairs will shift up while the targeted pair moves down.
    ///
    /// ***Panics*** if `to` is out of bounds.
    ///
    /// Computes in **O(n)** time (average).
    #[track_caller]
    pub fn move_index(self, to: usize) {
        let index = self.index();
        self.into_ref_mut().move_index(index, to);
    }

    /// Swaps the position of entry with another.
    ///
    /// This is equivalent to [`IndexMap::swap_indices`]
    /// with the current [`.index()`][Self::index] as one of the two being swapped.
    ///
    /// ***Panics*** if the `other` index is out of bounds.
    ///
    /// Computes in **O(1)** time (average).
    #[track_caller]
    pub fn swap_indices(self, other: usize) {
        let index = self.index();
        self.into_ref_mut().swap_indices(index, other);
    }
}

/// A view into a vacant raw entry in an [`IndexMap`].
/// It is part of the [`RawEntryMut`] enum.
pub struct RawVacantEntryMut<'a, K, V, S> {
    map: RefMut<'a, K, V>,
    hash_builder: &'a S,
}

impl<K, V, S> fmt::Debug for RawVacantEntryMut<'_, K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawVacantEntryMut").finish_non_exhaustive()
    }
}

impl<'a, K, V, S> RawVacantEntryMut<'a, K, V, S> {
    /// Return the index where a key-value pair may be inserted.
    pub fn index(&self) -> usize {
        self.map.indices.len()
    }

    /// Inserts the given key and value into the map,
    /// and returns mutable references to them.
    pub fn insert(self, key: K, value: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
        S: BuildHasher,
    {
        let h = self.hash_builder.hash_one(&key);
        self.insert_hashed_nocheck(h, key, value)
    }

    /// Inserts the given key and value into the map with the provided hash,
    /// and returns mutable references to them.
    pub fn insert_hashed_nocheck(self, hash: u64, key: K, value: V) -> (&'a mut K, &'a mut V) {
        let hash = HashValue(hash as usize);
        self.map.insert_unique(hash, key, value).into_muts()
    }

    /// Inserts the given key and value into the map at the given index,
    /// shifting others to the right, and returns mutable references to them.
    ///
    /// ***Panics*** if `index` is out of bounds.
    ///
    /// Computes in **O(n)** time (average).
    #[track_caller]
    pub fn shift_insert(self, index: usize, key: K, value: V) -> (&'a mut K, &'a mut V)
    where
        K: Hash,
        S: BuildHasher,
    {
        let h = self.hash_builder.hash_one(&key);
        self.shift_insert_hashed_nocheck(index, h, key, value)
    }

    /// Inserts the given key and value into the map with the provided hash
    /// at the given index, and returns mutable references to them.
    ///
    /// ***Panics*** if `index` is out of bounds.
    ///
    /// Computes in **O(n)** time (average).
    #[track_caller]
    pub fn shift_insert_hashed_nocheck(
        mut self,
        index: usize,
        hash: u64,
        key: K,
        value: V,
    ) -> (&'a mut K, &'a mut V) {
        let hash = HashValue(hash as usize);
        self.map.shift_insert_unique(index, hash, key, value);
        self.map.entries[index].muts()
    }
}

trait Sealed {}

impl<K, V, S> Sealed for IndexMap<K, V, S> {}
