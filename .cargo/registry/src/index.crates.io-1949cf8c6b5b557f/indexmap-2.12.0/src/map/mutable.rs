use core::hash::{BuildHasher, Hash};

use super::{
    Bucket, Entry, Equivalent, IndexMap, IndexedEntry, IterMut2, OccupiedEntry, VacantEntry,
};

/// Opt-in mutable access to [`IndexMap`] keys.
///
/// These methods expose `&mut K`, mutable references to the key as it is stored
/// in the map.
/// You are allowed to modify the keys in the map **if the modification
/// does not change the key's hash and equality**.
///
/// If keys are modified erroneously, you can no longer look them up.
/// This is sound (memory safe) but a logical error hazard (just like
/// implementing `PartialEq`, `Eq`, or `Hash` incorrectly would be).
///
/// `use` this trait to enable its methods for `IndexMap`.
///
/// This trait is sealed and cannot be implemented for types outside this crate.
#[expect(private_bounds)]
pub trait MutableKeys: Sealed {
    type Key;
    type Value;

    /// Return item index, mutable reference to key and value
    ///
    /// Computes in **O(1)** time (average).
    fn get_full_mut2<Q>(&mut self, key: &Q) -> Option<(usize, &mut Self::Key, &mut Self::Value)>
    where
        Q: ?Sized + Hash + Equivalent<Self::Key>;

    /// Return mutable reference to key and value at an index.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    fn get_index_mut2(&mut self, index: usize) -> Option<(&mut Self::Key, &mut Self::Value)>;

    /// Return an iterator over the key-value pairs of the map, in their order
    fn iter_mut2(&mut self) -> IterMut2<'_, Self::Key, Self::Value>;

    /// Scan through each key-value pair in the map and keep those where the
    /// closure `keep` returns `true`.
    ///
    /// The elements are visited in order, and remaining elements keep their
    /// order.
    ///
    /// Computes in **O(n)** time (average).
    fn retain2<F>(&mut self, keep: F)
    where
        F: FnMut(&mut Self::Key, &mut Self::Value) -> bool;
}

/// Opt-in mutable access to [`IndexMap`] keys.
///
/// See [`MutableKeys`] for more information.
impl<K, V, S> MutableKeys for IndexMap<K, V, S>
where
    S: BuildHasher,
{
    type Key = K;
    type Value = V;

    fn get_full_mut2<Q>(&mut self, key: &Q) -> Option<(usize, &mut K, &mut V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        if let Some(i) = self.get_index_of(key) {
            let entry = &mut self.as_entries_mut()[i];
            Some((i, &mut entry.key, &mut entry.value))
        } else {
            None
        }
    }

    fn get_index_mut2(&mut self, index: usize) -> Option<(&mut K, &mut V)> {
        self.as_entries_mut().get_mut(index).map(Bucket::muts)
    }

    fn iter_mut2(&mut self) -> IterMut2<'_, Self::Key, Self::Value> {
        IterMut2::new(self.as_entries_mut())
    }

    fn retain2<F>(&mut self, keep: F)
    where
        F: FnMut(&mut K, &mut V) -> bool,
    {
        self.core.retain_in_order(keep);
    }
}

/// Opt-in mutable access to [`Entry`] keys.
///
/// These methods expose `&mut K`, mutable references to the key as it is stored
/// in the map.
/// You are allowed to modify the keys in the map **if the modification
/// does not change the key's hash and equality**.
///
/// If keys are modified erroneously, you can no longer look them up.
/// This is sound (memory safe) but a logical error hazard (just like
/// implementing `PartialEq`, `Eq`, or `Hash` incorrectly would be).
///
/// `use` this trait to enable its methods for `Entry`.
///
/// This trait is sealed and cannot be implemented for types outside this crate.
#[expect(private_bounds)]
pub trait MutableEntryKey: Sealed {
    type Key;

    /// Gets a mutable reference to the entry's key, either within the map if occupied,
    /// or else the new key that was used to find the entry.
    fn key_mut(&mut self) -> &mut Self::Key;
}

/// Opt-in mutable access to [`Entry`] keys.
///
/// See [`MutableEntryKey`] for more information.
impl<K, V> MutableEntryKey for Entry<'_, K, V> {
    type Key = K;
    fn key_mut(&mut self) -> &mut Self::Key {
        match self {
            Entry::Occupied(e) => e.key_mut(),
            Entry::Vacant(e) => e.key_mut(),
        }
    }
}

/// Opt-in mutable access to [`OccupiedEntry`] keys.
///
/// See [`MutableEntryKey`] for more information.
impl<K, V> MutableEntryKey for OccupiedEntry<'_, K, V> {
    type Key = K;
    fn key_mut(&mut self) -> &mut Self::Key {
        self.key_mut()
    }
}

/// Opt-in mutable access to [`VacantEntry`] keys.
///
/// See [`MutableEntryKey`] for more information.
impl<K, V> MutableEntryKey for VacantEntry<'_, K, V> {
    type Key = K;
    fn key_mut(&mut self) -> &mut Self::Key {
        self.key_mut()
    }
}

/// Opt-in mutable access to [`IndexedEntry`] keys.
///
/// See [`MutableEntryKey`] for more information.
impl<K, V> MutableEntryKey for IndexedEntry<'_, K, V> {
    type Key = K;
    fn key_mut(&mut self) -> &mut Self::Key {
        self.key_mut()
    }
}

trait Sealed {}

impl<K, V, S> Sealed for IndexMap<K, V, S> {}
impl<K, V> Sealed for Entry<'_, K, V> {}
impl<K, V> Sealed for OccupiedEntry<'_, K, V> {}
impl<K, V> Sealed for VacantEntry<'_, K, V> {}
impl<K, V> Sealed for IndexedEntry<'_, K, V> {}
