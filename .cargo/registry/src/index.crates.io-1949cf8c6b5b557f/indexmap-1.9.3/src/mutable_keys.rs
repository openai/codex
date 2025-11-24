use core::hash::{BuildHasher, Hash};

use super::{Equivalent, IndexMap};

pub struct PrivateMarker {}

/// Opt-in mutable access to keys.
///
/// These methods expose `&mut K`, mutable references to the key as it is stored
/// in the map.
/// You are allowed to modify the keys in the hashmap **if the modification
/// does not change the key’s hash and equality**.
///
/// If keys are modified erroneously, you can no longer look them up.
/// This is sound (memory safe) but a logical error hazard (just like
/// implementing PartialEq, Eq, or Hash incorrectly would be).
///
/// `use` this trait to enable its methods for `IndexMap`.
pub trait MutableKeys {
    type Key;
    type Value;

    /// Return item index, mutable reference to key and value
    fn get_full_mut2<Q: ?Sized>(
        &mut self,
        key: &Q,
    ) -> Option<(usize, &mut Self::Key, &mut Self::Value)>
    where
        Q: Hash + Equivalent<Self::Key>;

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

    /// This method is not useful in itself – it is there to “seal” the trait
    /// for external implementation, so that we can add methods without
    /// causing breaking changes.
    fn __private_marker(&self) -> PrivateMarker;
}

/// Opt-in mutable access to keys.
///
/// See [`MutableKeys`](trait.MutableKeys.html) for more information.
impl<K, V, S> MutableKeys for IndexMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    type Key = K;
    type Value = V;
    fn get_full_mut2<Q: ?Sized>(&mut self, key: &Q) -> Option<(usize, &mut K, &mut V)>
    where
        Q: Hash + Equivalent<K>,
    {
        self.get_full_mut2_impl(key)
    }

    fn retain2<F>(&mut self, keep: F)
    where
        F: FnMut(&mut K, &mut V) -> bool,
    {
        self.retain_mut(keep)
    }

    fn __private_marker(&self) -> PrivateMarker {
        PrivateMarker {}
    }
}
