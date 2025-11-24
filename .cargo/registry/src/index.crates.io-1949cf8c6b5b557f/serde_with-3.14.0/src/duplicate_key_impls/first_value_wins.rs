use crate::prelude::*;

pub trait DuplicateInsertsFirstWinsMap<K, V> {
    fn new(size_hint: Option<usize>) -> Self;

    /// Insert the value into the map, if there is not already an existing value
    fn insert(&mut self, key: K, value: V);
}

#[cfg(feature = "std")]
impl<K, V, S> DuplicateInsertsFirstWinsMap<K, V> for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn new(size_hint: Option<usize>) -> Self {
        match size_hint {
            Some(size) => Self::with_capacity_and_hasher(size, S::default()),
            None => Self::with_hasher(S::default()),
        }
    }

    #[inline]
    fn insert(&mut self, key: K, value: V) {
        use std::collections::hash_map::Entry;

        match self.entry(key) {
            // we want to keep the first value, so do nothing
            Entry::Occupied(_) => {}
            Entry::Vacant(vacant) => {
                vacant.insert(value);
            }
        }
    }
}

#[cfg(feature = "hashbrown_0_14")]
impl<K, V, S> DuplicateInsertsFirstWinsMap<K, V> for hashbrown_0_14::HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn new(size_hint: Option<usize>) -> Self {
        match size_hint {
            Some(size) => Self::with_capacity_and_hasher(size, S::default()),
            None => Self::with_hasher(S::default()),
        }
    }

    #[inline]
    fn insert(&mut self, key: K, value: V) {
        use hashbrown_0_14::hash_map::Entry;

        match self.entry(key) {
            // we want to keep the first value, so do nothing
            Entry::Occupied(_) => {}
            Entry::Vacant(vacant) => {
                vacant.insert(value);
            }
        }
    }
}

#[cfg(feature = "hashbrown_0_15")]
impl<K, V, S> DuplicateInsertsFirstWinsMap<K, V> for hashbrown_0_15::HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn new(size_hint: Option<usize>) -> Self {
        match size_hint {
            Some(size) => Self::with_capacity_and_hasher(size, S::default()),
            None => Self::with_hasher(S::default()),
        }
    }

    #[inline]
    fn insert(&mut self, key: K, value: V) {
        use hashbrown_0_15::hash_map::Entry;

        match self.entry(key) {
            // we want to keep the first value, so do nothing
            Entry::Occupied(_) => {}
            Entry::Vacant(vacant) => {
                vacant.insert(value);
            }
        }
    }
}

#[cfg(feature = "indexmap_1")]
impl<K, V, S> DuplicateInsertsFirstWinsMap<K, V> for indexmap_1::IndexMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn new(size_hint: Option<usize>) -> Self {
        match size_hint {
            Some(size) => Self::with_capacity_and_hasher(size, S::default()),
            None => Self::with_hasher(S::default()),
        }
    }

    #[inline]
    fn insert(&mut self, key: K, value: V) {
        use indexmap_1::map::Entry;

        match self.entry(key) {
            // we want to keep the first value, so do nothing
            Entry::Occupied(_) => {}
            Entry::Vacant(vacant) => {
                vacant.insert(value);
            }
        }
    }
}

#[cfg(feature = "indexmap_2")]
impl<K, V, S> DuplicateInsertsFirstWinsMap<K, V> for indexmap_2::IndexMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn new(size_hint: Option<usize>) -> Self {
        match size_hint {
            Some(size) => Self::with_capacity_and_hasher(size, S::default()),
            None => Self::with_hasher(S::default()),
        }
    }

    #[inline]
    fn insert(&mut self, key: K, value: V) {
        use indexmap_2::map::Entry;

        match self.entry(key) {
            // we want to keep the first value, so do nothing
            Entry::Occupied(_) => {}
            Entry::Vacant(vacant) => {
                vacant.insert(value);
            }
        }
    }
}

impl<K, V> DuplicateInsertsFirstWinsMap<K, V> for BTreeMap<K, V>
where
    K: Ord,
{
    #[inline]
    fn new(_size_hint: Option<usize>) -> Self {
        Self::new()
    }

    #[inline]
    fn insert(&mut self, key: K, value: V) {
        use alloc::collections::btree_map::Entry;

        match self.entry(key) {
            // we want to keep the first value, so do nothing
            Entry::Occupied(_) => {}
            Entry::Vacant(vacant) => {
                vacant.insert(value);
            }
        }
    }
}
