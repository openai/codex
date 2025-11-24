// Copyright 2017 Serde Developers
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A map of `String` to [Value][crate::Value].
//!
//! By default the map is backed by a [`BTreeMap`]. Enable the `preserve_order`
//! feature of toml-rs to use [`IndexMap`] instead.
//!
//! [`BTreeMap`]: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html
//! [`IndexMap`]: https://docs.rs/indexmap

#[cfg(not(feature = "preserve_order"))]
use alloc::collections::{btree_map, BTreeMap};
use core::borrow::Borrow;
use core::fmt::{self, Debug};
use core::hash::Hash;
use core::iter::FromIterator;
use core::ops;

#[cfg(feature = "preserve_order")]
use indexmap::{self, IndexMap};

/// Represents a TOML key/value type.
pub struct Map<K, V> {
    map: MapImpl<K, V>,
    dotted: bool,
    implicit: bool,
    inline: bool,
}

#[cfg(not(feature = "preserve_order"))]
type MapImpl<K, V> = BTreeMap<K, V>;
#[cfg(all(feature = "preserve_order", not(feature = "fast_hash")))]
type RandomState = std::collections::hash_map::RandomState;
#[cfg(all(feature = "preserve_order", feature = "fast_hash"))]
type RandomState = foldhash::fast::RandomState;
#[cfg(feature = "preserve_order")]
type MapImpl<K, V> = IndexMap<K, V, RandomState>;

impl<K, V> Map<K, V>
where
    K: Ord + Hash,
{
    /// Makes a new empty Map.
    #[inline]
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "preserve_order")]
            map: MapImpl::with_hasher(RandomState::default()),
            #[cfg(not(feature = "preserve_order"))]
            map: MapImpl::new(),
            dotted: false,
            implicit: false,
            inline: false,
        }
    }

    #[cfg(not(feature = "preserve_order"))]
    /// Makes a new empty Map with the given initial capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        // does not support with_capacity
        let _ = capacity;
        Self::new()
    }

    #[cfg(feature = "preserve_order")]
    /// Makes a new empty Map with the given initial capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            map: IndexMap::with_capacity_and_hasher(capacity, RandomState::default()),
            dotted: false,
            implicit: false,
            inline: false,
        }
    }

    /// Clears the map, removing all values.
    #[inline]
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Returns a reference to the value corresponding to the key.
    ///
    /// The key may be any borrowed form of the map's key type, but the ordering
    /// on the borrowed form *must* match the ordering on the key type.
    #[inline]
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + Eq + Hash + ?Sized,
    {
        self.map.get(key)
    }

    /// Returns true if the map contains a value for the specified key.
    ///
    /// The key may be any borrowed form of the map's key type, but the ordering
    /// on the borrowed form *must* match the ordering on the key type.
    #[inline]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Ord + Eq + Hash + ?Sized,
    {
        self.map.contains_key(key)
    }

    /// Returns a mutable reference to the value corresponding to the key.
    ///
    /// The key may be any borrowed form of the map's key type, but the ordering
    /// on the borrowed form *must* match the ordering on the key type.
    #[inline]
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Ord + Eq + Hash + ?Sized,
    {
        self.map.get_mut(key)
    }

    /// Returns the key-value pair matching the given key.
    ///
    /// The key may be any borrowed form of the map's key type, but the ordering
    /// on the borrowed form *must* match the ordering on the key type.
    #[inline]
    pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: ?Sized + Ord + Eq + Hash,
    {
        self.map.get_key_value(key)
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the map did not have this key present, `None` is returned.
    ///
    /// If the map did have this key present, the value is updated, and the old
    /// value is returned. The key is not updated, though; this matters for
    /// types that can be `==` without being identical.
    #[inline]
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.map.insert(k, v)
    }

    /// Removes a key from the map, returning the value at the key if the key
    /// was previously in the map.
    ///
    /// The key may be any borrowed form of the map's key type, but the ordering
    /// on the borrowed form *must* match the ordering on the key type.
    #[inline]
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Ord + Eq + Hash + ?Sized,
    {
        #[cfg(not(feature = "preserve_order"))]
        {
            self.map.remove(key)
        }
        #[cfg(feature = "preserve_order")]
        {
            self.map.shift_remove(key)
        }
    }

    /// Removes a key from the map, returning the stored key and value if the key was previously in the map.
    #[inline]
    pub fn remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Ord + Eq + Hash + ?Sized,
    {
        #[cfg(not(feature = "preserve_order"))]
        {
            self.map.remove_entry(key)
        }
        #[cfg(feature = "preserve_order")]
        {
            self.map.shift_remove_entry(key)
        }
    }

    /// Retains only the elements specified by the `keep` predicate.
    ///
    /// In other words, remove all pairs `(k, v)` for which `keep(&k, &mut v)`
    /// returns `false`.
    ///
    /// The elements are visited in iteration order.
    #[inline]
    pub fn retain<F>(&mut self, mut keep: F)
    where
        K: AsRef<str>,
        F: FnMut(&str, &mut V) -> bool,
    {
        self.map.retain(|key, value| keep(key.as_ref(), value));
    }

    /// Gets the given key's corresponding entry in the map for in-place
    /// manipulation.
    pub fn entry<S>(&mut self, key: S) -> Entry<'_, K, V>
    where
        S: Into<K>,
    {
        #[cfg(not(feature = "preserve_order"))]
        use alloc::collections::btree_map::Entry as EntryImpl;
        #[cfg(feature = "preserve_order")]
        use indexmap::map::Entry as EntryImpl;

        match self.map.entry(key.into()) {
            EntryImpl::Vacant(vacant) => Entry::Vacant(VacantEntry { vacant }),
            EntryImpl::Occupied(occupied) => Entry::Occupied(OccupiedEntry { occupied }),
        }
    }

    /// Returns the number of elements in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns true if the map contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Gets an iterator over the entries of the map.
    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            iter: self.map.iter(),
        }
    }

    /// Gets a mutable iterator over the entries of the map.
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut {
            iter: self.map.iter_mut(),
        }
    }

    /// Gets an iterator over the keys of the map.
    #[inline]
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys {
            iter: self.map.keys(),
        }
    }

    /// Gets an iterator over the values of the map.
    #[inline]
    pub fn values(&self) -> Values<'_, K, V> {
        Values {
            iter: self.map.values(),
        }
    }

    /// Scan through each key-value pair in the map and keep those where the
    /// closure `keep` returns `true`.
    ///
    /// The elements are visited in order, and remaining elements keep their
    /// order.
    ///
    /// Computes in **O(n)** time (average).
    #[allow(unused_mut)]
    pub(crate) fn mut_entries<F>(&mut self, mut op: F)
    where
        F: FnMut(&mut K, &mut V),
    {
        #[cfg(feature = "preserve_order")]
        {
            use indexmap::map::MutableKeys as _;
            for (key, value) in self.map.iter_mut2() {
                op(key, value);
            }
        }
        #[cfg(not(feature = "preserve_order"))]
        {
            self.map = core::mem::take(&mut self.map)
                .into_iter()
                .map(move |(mut k, mut v)| {
                    op(&mut k, &mut v);
                    (k, v)
                })
                .collect();
        }
    }
}

impl<K, V> Map<K, V>
where
    K: Ord,
{
    pub(crate) fn is_dotted(&self) -> bool {
        self.dotted
    }

    pub(crate) fn is_implicit(&self) -> bool {
        self.implicit
    }

    pub(crate) fn is_inline(&self) -> bool {
        self.inline
    }

    pub(crate) fn set_implicit(&mut self, yes: bool) {
        self.implicit = yes;
    }

    pub(crate) fn set_dotted(&mut self, yes: bool) {
        self.dotted = yes;
    }

    pub(crate) fn set_inline(&mut self, yes: bool) {
        self.inline = yes;
    }
}

impl<K, V> Default for Map<K, V>
where
    K: Ord + Hash,
{
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Clone, V: Clone> Clone for Map<K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
            dotted: self.dotted,
            implicit: self.implicit,
            inline: self.inline,
        }
    }
}

impl<K: Eq + Hash, V: PartialEq> PartialEq for Map<K, V> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.map.eq(&other.map)
    }
}

/// Access an element of this map. Panics if the given key is not present in the
/// map.
impl<K, V, Q> ops::Index<&Q> for Map<K, V>
where
    K: Borrow<Q> + Ord,
    Q: Ord + Eq + Hash + ?Sized,
{
    type Output = V;

    fn index(&self, index: &Q) -> &V {
        self.map.index(index)
    }
}

/// Mutably access an element of this map. Panics if the given key is not
/// present in the map.
impl<K, V, Q> ops::IndexMut<&Q> for Map<K, V>
where
    K: Borrow<Q> + Ord,
    Q: Ord + Eq + Hash + ?Sized,
{
    fn index_mut(&mut self, index: &Q) -> &mut V {
        self.map.get_mut(index).expect("no entry found for key")
    }
}

impl<K: Debug, V: Debug> Debug for Map<K, V> {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        self.map.fmt(formatter)
    }
}

impl<K: Ord + Hash, V> FromIterator<(K, V)> for Map<K, V> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = (K, V)>,
    {
        Self {
            map: FromIterator::from_iter(iter),
            dotted: false,
            implicit: false,
            inline: false,
        }
    }
}

impl<K: Ord + Hash, V> Extend<(K, V)> for Map<K, V> {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (K, V)>,
    {
        self.map.extend(iter);
    }
}

macro_rules! delegate_iterator {
    (($name:ident $($generics:tt)*) => $item:ty) => {
        impl $($generics)* Iterator for $name $($generics)* {
            type Item = $item;
            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.iter.next()
            }
            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.iter.size_hint()
            }
        }

        impl $($generics)* DoubleEndedIterator for $name $($generics)* {
            #[inline]
            fn next_back(&mut self) -> Option<Self::Item> {
                self.iter.next_back()
            }
        }

        impl $($generics)* ExactSizeIterator for $name $($generics)* {
            #[inline]
            fn len(&self) -> usize {
                self.iter.len()
            }
        }
    }
}

//////////////////////////////////////////////////////////////////////////////

/// A view into a single entry in a map, which may either be vacant or occupied.
/// This enum is constructed from the [`entry`] method on [`Map`].
///
/// [`entry`]: struct.Map.html#method.entry
/// [`Map`]: struct.Map.html
pub enum Entry<'a, K, V> {
    /// A vacant Entry.
    Vacant(VacantEntry<'a, K, V>),
    /// An occupied Entry.
    Occupied(OccupiedEntry<'a, K, V>),
}

/// A vacant Entry. It is part of the [`Entry`] enum.
///
/// [`Entry`]: enum.Entry.html
pub struct VacantEntry<'a, K, V> {
    vacant: VacantEntryImpl<'a, K, V>,
}

/// An occupied Entry. It is part of the [`Entry`] enum.
///
/// [`Entry`]: enum.Entry.html
pub struct OccupiedEntry<'a, K, V> {
    occupied: OccupiedEntryImpl<'a, K, V>,
}

#[cfg(not(feature = "preserve_order"))]
type VacantEntryImpl<'a, K, V> = btree_map::VacantEntry<'a, K, V>;
#[cfg(feature = "preserve_order")]
type VacantEntryImpl<'a, K, V> = indexmap::map::VacantEntry<'a, K, V>;

#[cfg(not(feature = "preserve_order"))]
type OccupiedEntryImpl<'a, K, V> = btree_map::OccupiedEntry<'a, K, V>;
#[cfg(feature = "preserve_order")]
type OccupiedEntryImpl<'a, K, V> = indexmap::map::OccupiedEntry<'a, K, V>;

impl<'a, K: Ord, V> Entry<'a, K, V> {
    /// Returns a reference to this entry's key.
    pub fn key(&self) -> &K {
        match *self {
            Entry::Vacant(ref e) => e.key(),
            Entry::Occupied(ref e) => e.key(),
        }
    }

    /// Ensures a value is in the entry by inserting the default if empty, and
    /// returns a mutable reference to the value in the entry.
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Entry::Vacant(entry) => entry.insert(default),
            Entry::Occupied(entry) => entry.into_mut(),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default
    /// function if empty, and returns a mutable reference to the value in the
    /// entry.
    pub fn or_insert_with<F>(self, default: F) -> &'a mut V
    where
        F: FnOnce() -> V,
    {
        match self {
            Entry::Vacant(entry) => entry.insert(default()),
            Entry::Occupied(entry) => entry.into_mut(),
        }
    }
}

impl<'a, K: Ord, V> VacantEntry<'a, K, V> {
    /// Gets a reference to the key that would be used when inserting a value
    /// through the `VacantEntry`.
    #[inline]
    pub fn key(&self) -> &K {
        self.vacant.key()
    }

    /// Sets the value of the entry with the `VacantEntry`'s key, and returns a
    /// mutable reference to it.
    #[inline]
    pub fn insert(self, value: V) -> &'a mut V {
        self.vacant.insert(value)
    }
}

impl<'a, K: Ord, V> OccupiedEntry<'a, K, V> {
    /// Gets a reference to the key in the entry.
    #[inline]
    pub fn key(&self) -> &K {
        self.occupied.key()
    }

    /// Gets a reference to the value in the entry.
    #[inline]
    pub fn get(&self) -> &V {
        self.occupied.get()
    }

    /// Gets a mutable reference to the value in the entry.
    #[inline]
    pub fn get_mut(&mut self) -> &mut V {
        self.occupied.get_mut()
    }

    /// Converts the entry into a mutable reference to its value.
    #[inline]
    pub fn into_mut(self) -> &'a mut V {
        self.occupied.into_mut()
    }

    /// Sets the value of the entry with the `OccupiedEntry`'s key, and returns
    /// the entry's old value.
    #[inline]
    pub fn insert(&mut self, value: V) -> V {
        self.occupied.insert(value)
    }

    /// Takes the value of the entry out of the map, and returns it.
    #[inline]
    pub fn remove(self) -> V {
        #[cfg(not(feature = "preserve_order"))]
        {
            self.occupied.remove()
        }
        #[cfg(feature = "preserve_order")]
        {
            self.occupied.shift_remove()
        }
    }
}

//////////////////////////////////////////////////////////////////////////////

impl<'a, K, V> IntoIterator for &'a Map<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        Iter {
            iter: self.map.iter(),
        }
    }
}

/// An iterator over a `toml::Map`'s entries.
pub struct Iter<'a, K, V> {
    iter: IterImpl<'a, K, V>,
}

#[cfg(not(feature = "preserve_order"))]
type IterImpl<'a, K, V> = btree_map::Iter<'a, K, V>;
#[cfg(feature = "preserve_order")]
type IterImpl<'a, K, V> = indexmap::map::Iter<'a, K, V>;

delegate_iterator!((Iter<'a, K, V>) => (&'a K, &'a V));

//////////////////////////////////////////////////////////////////////////////

impl<'a, K, V> IntoIterator for &'a mut Map<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IterMut {
            iter: self.map.iter_mut(),
        }
    }
}

/// A mutable iterator over a `toml::Map`'s entries.
pub struct IterMut<'a, K, V> {
    iter: IterMutImpl<'a, K, V>,
}

#[cfg(not(feature = "preserve_order"))]
type IterMutImpl<'a, K, V> = btree_map::IterMut<'a, K, V>;
#[cfg(feature = "preserve_order")]
type IterMutImpl<'a, K, V> = indexmap::map::IterMut<'a, K, V>;

delegate_iterator!((IterMut<'a, K, V>) => (&'a K, &'a mut V));

//////////////////////////////////////////////////////////////////////////////

impl<K, V> IntoIterator for Map<K, V> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            iter: self.map.into_iter(),
        }
    }
}

/// An owning iterator over a `toml::Map`'s entries.
pub struct IntoIter<K, V> {
    iter: IntoIterImpl<K, V>,
}

#[cfg(not(feature = "preserve_order"))]
type IntoIterImpl<K, V> = btree_map::IntoIter<K, V>;
#[cfg(feature = "preserve_order")]
type IntoIterImpl<K, V> = indexmap::map::IntoIter<K, V>;

delegate_iterator!((IntoIter<K,V>) => (K, V));

//////////////////////////////////////////////////////////////////////////////

/// An iterator over a `toml::Map`'s keys.
pub struct Keys<'a, K, V> {
    iter: KeysImpl<'a, K, V>,
}

#[cfg(not(feature = "preserve_order"))]
type KeysImpl<'a, K, V> = btree_map::Keys<'a, K, V>;
#[cfg(feature = "preserve_order")]
type KeysImpl<'a, K, V> = indexmap::map::Keys<'a, K, V>;

delegate_iterator!((Keys<'a, K, V>) => &'a K);

//////////////////////////////////////////////////////////////////////////////

/// An iterator over a `toml::Map`'s values.
pub struct Values<'a, K, V> {
    iter: ValuesImpl<'a, K, V>,
}

#[cfg(not(feature = "preserve_order"))]
type ValuesImpl<'a, K, V> = btree_map::Values<'a, K, V>;
#[cfg(feature = "preserve_order")]
type ValuesImpl<'a, K, V> = indexmap::map::Values<'a, K, V>;

delegate_iterator!((Values<'a, K, V>) => &'a V);
