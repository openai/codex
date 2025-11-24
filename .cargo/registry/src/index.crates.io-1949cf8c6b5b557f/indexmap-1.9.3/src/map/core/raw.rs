#![allow(unsafe_code)]
//! This module encapsulates the `unsafe` access to `hashbrown::raw::RawTable`,
//! mostly in dealing with its bucket "pointers".

use super::{equivalent, Bucket, Entry, HashValue, IndexMapCore, VacantEntry};
use core::fmt;
use core::mem::replace;
use hashbrown::raw::RawTable;

type RawBucket = hashbrown::raw::Bucket<usize>;

/// Inserts many entries into a raw table without reallocating.
///
/// ***Panics*** if there is not sufficient capacity already.
pub(super) fn insert_bulk_no_grow<K, V>(indices: &mut RawTable<usize>, entries: &[Bucket<K, V>]) {
    assert!(indices.capacity() - indices.len() >= entries.len());
    for entry in entries {
        // SAFETY: we asserted that sufficient capacity exists for all entries.
        unsafe {
            indices.insert_no_grow(entry.hash.get(), indices.len());
        }
    }
}

pub(super) struct DebugIndices<'a>(pub &'a RawTable<usize>);
impl fmt::Debug for DebugIndices<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: we're not letting any of the buckets escape this function
        let indices = unsafe { self.0.iter().map(|raw_bucket| raw_bucket.read()) };
        f.debug_list().entries(indices).finish()
    }
}

impl<K, V> IndexMapCore<K, V> {
    /// Sweep the whole table to erase indices start..end
    pub(super) fn erase_indices_sweep(&mut self, start: usize, end: usize) {
        // SAFETY: we're not letting any of the buckets escape this function
        unsafe {
            let offset = end - start;
            for bucket in self.indices.iter() {
                let i = bucket.read();
                if i >= end {
                    bucket.write(i - offset);
                } else if i >= start {
                    self.indices.erase(bucket);
                }
            }
        }
    }

    pub(crate) fn entry(&mut self, hash: HashValue, key: K) -> Entry<'_, K, V>
    where
        K: Eq,
    {
        let eq = equivalent(&key, &self.entries);
        match self.indices.find(hash.get(), eq) {
            // SAFETY: The entry is created with a live raw bucket, at the same time
            // we have a &mut reference to the map, so it can not be modified further.
            Some(raw_bucket) => Entry::Occupied(OccupiedEntry {
                map: self,
                raw_bucket,
                key,
            }),
            None => Entry::Vacant(VacantEntry {
                map: self,
                hash,
                key,
            }),
        }
    }

    pub(super) fn indices_mut(&mut self) -> impl Iterator<Item = &mut usize> {
        // SAFETY: we're not letting any of the buckets escape this function,
        // only the item references that are appropriately bound to `&mut self`.
        unsafe { self.indices.iter().map(|bucket| bucket.as_mut()) }
    }

    /// Return the raw bucket for the given index
    fn find_index(&self, index: usize) -> RawBucket {
        // We'll get a "nice" bounds-check from indexing `self.entries`,
        // and then we expect to find it in the table as well.
        let hash = self.entries[index].hash.get();
        self.indices
            .find(hash, move |&i| i == index)
            .expect("index not found")
    }

    pub(crate) fn swap_indices(&mut self, a: usize, b: usize) {
        // SAFETY: Can't take two `get_mut` references from one table, so we
        // must use raw buckets to do the swap. This is still safe because we
        // are locally sure they won't dangle, and we write them individually.
        unsafe {
            let raw_bucket_a = self.find_index(a);
            let raw_bucket_b = self.find_index(b);
            raw_bucket_a.write(b);
            raw_bucket_b.write(a);
        }
        self.entries.swap(a, b);
    }
}

/// A view into an occupied entry in a `IndexMap`.
/// It is part of the [`Entry`] enum.
///
/// [`Entry`]: enum.Entry.html
// SAFETY: The lifetime of the map reference also constrains the raw bucket,
// which is essentially a raw pointer into the map indices.
pub struct OccupiedEntry<'a, K, V> {
    map: &'a mut IndexMapCore<K, V>,
    raw_bucket: RawBucket,
    key: K,
}

// `hashbrown::raw::Bucket` is only `Send`, not `Sync`.
// SAFETY: `&self` only accesses the bucket to read it.
unsafe impl<K: Sync, V: Sync> Sync for OccupiedEntry<'_, K, V> {}

// The parent module also adds methods that don't threaten the unsafe encapsulation.
impl<'a, K, V> OccupiedEntry<'a, K, V> {
    /// Gets a reference to the entry's key in the map.
    ///
    /// Note that this is not the key that was used to find the entry. There may be an observable
    /// difference if the key type has any distinguishing features outside of `Hash` and `Eq`, like
    /// extra fields or the memory address of an allocation.
    pub fn key(&self) -> &K {
        &self.map.entries[self.index()].key
    }

    /// Gets a reference to the entry's value in the map.
    pub fn get(&self) -> &V {
        &self.map.entries[self.index()].value
    }

    /// Gets a mutable reference to the entry's value in the map.
    ///
    /// If you need a reference which may outlive the destruction of the
    /// `Entry` value, see `into_mut`.
    pub fn get_mut(&mut self) -> &mut V {
        let index = self.index();
        &mut self.map.entries[index].value
    }

    /// Put the new key in the occupied entry's key slot
    pub(crate) fn replace_key(self) -> K {
        let index = self.index();
        let old_key = &mut self.map.entries[index].key;
        replace(old_key, self.key)
    }

    /// Return the index of the key-value pair
    #[inline]
    pub fn index(&self) -> usize {
        // SAFETY: we have &mut map keep keeping the bucket stable
        unsafe { self.raw_bucket.read() }
    }

    /// Converts into a mutable reference to the entry's value in the map,
    /// with a lifetime bound to the map itself.
    pub fn into_mut(self) -> &'a mut V {
        let index = self.index();
        &mut self.map.entries[index].value
    }

    /// Remove and return the key, value pair stored in the map for this entry
    ///
    /// Like `Vec::swap_remove`, the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Computes in **O(1)** time (average).
    pub fn swap_remove_entry(self) -> (K, V) {
        // SAFETY: This is safe because it can only happen once (self is consumed)
        // and map.indices have not been modified since entry construction
        let index = unsafe { self.map.indices.remove(self.raw_bucket) };
        self.map.swap_remove_finish(index)
    }

    /// Remove and return the key, value pair stored in the map for this entry
    ///
    /// Like `Vec::remove`, the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Computes in **O(n)** time (average).
    pub fn shift_remove_entry(self) -> (K, V) {
        // SAFETY: This is safe because it can only happen once (self is consumed)
        // and map.indices have not been modified since entry construction
        let index = unsafe { self.map.indices.remove(self.raw_bucket) };
        self.map.shift_remove_finish(index)
    }
}
