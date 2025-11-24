use core::hash::{BuildHasher, Hash};

use super::{Equivalent, IndexSet};
use crate::map::MutableKeys;

/// Opt-in mutable access to [`IndexSet`] values.
///
/// These methods expose `&mut T`, mutable references to the value as it is stored
/// in the set.
/// You are allowed to modify the values in the set **if the modification
/// does not change the value's hash and equality**.
///
/// If values are modified erroneously, you can no longer look them up.
/// This is sound (memory safe) but a logical error hazard (just like
/// implementing `PartialEq`, `Eq`, or `Hash` incorrectly would be).
///
/// `use` this trait to enable its methods for `IndexSet`.
///
/// This trait is sealed and cannot be implemented for types outside this crate.
#[expect(private_bounds)]
pub trait MutableValues: Sealed {
    type Value;

    /// Return item index and mutable reference to the value
    ///
    /// Computes in **O(1)** time (average).
    fn get_full_mut2<Q>(&mut self, value: &Q) -> Option<(usize, &mut Self::Value)>
    where
        Q: ?Sized + Hash + Equivalent<Self::Value>;

    /// Return mutable reference to the value at an index.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    fn get_index_mut2(&mut self, index: usize) -> Option<&mut Self::Value>;

    /// Scan through each value in the set and keep those where the
    /// closure `keep` returns `true`.
    ///
    /// The values are visited in order, and remaining values keep their order.
    ///
    /// Computes in **O(n)** time (average).
    fn retain2<F>(&mut self, keep: F)
    where
        F: FnMut(&mut Self::Value) -> bool;
}

/// Opt-in mutable access to [`IndexSet`] values.
///
/// See [`MutableValues`] for more information.
impl<T, S> MutableValues for IndexSet<T, S>
where
    S: BuildHasher,
{
    type Value = T;

    fn get_full_mut2<Q>(&mut self, value: &Q) -> Option<(usize, &mut T)>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        match self.map.get_full_mut2(value) {
            Some((index, value, ())) => Some((index, value)),
            None => None,
        }
    }

    fn get_index_mut2(&mut self, index: usize) -> Option<&mut T> {
        match self.map.get_index_mut2(index) {
            Some((value, ())) => Some(value),
            None => None,
        }
    }

    fn retain2<F>(&mut self, mut keep: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        self.map.retain2(move |value, ()| keep(value));
    }
}

trait Sealed {}

impl<T, S> Sealed for IndexSet<T, S> {}
