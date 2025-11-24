// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Union-find implementation. The main type is `UnificationTable`.
//!
//! You can define your own type for the *keys* in the table, but you
//! must implement `UnifyKey` for that type. The assumption is that
//! keys will be newtyped integers, hence we require that they
//! implement `Copy`.
//!
//! Keys can have values associated with them. The assumption is that
//! these values are cheaply cloneable (ideally, `Copy`), and some of
//! the interfaces are oriented around that assumption. If you just
//! want the classical "union-find" algorithm where you group things
//! into sets, use the `Value` type of `()`.
//!
//! When you have keys with non-trivial values, you must also define
//! how those values can be merged. As part of doing this, you can
//! define the "error" type to return on error; if errors are not
//! possible, use `NoError` (an uninstantiable struct). Using this
//! type also unlocks various more ergonomic methods (e.g., `union()`
//! in place of `unify_var_var()`).
//!
//! The best way to see how it is used is to read the `tests.rs` file;
//! search for e.g. `UnitKey`.

use std::fmt::Debug;
use std::marker;
use std::ops::Range;

use snapshot_vec::{self as sv, UndoLog};
use undo_log::{UndoLogs, VecLog};

mod backing_vec;
pub use self::backing_vec::{
    Delegate, InPlace, UnificationStore, UnificationStoreBase, UnificationStoreMut,
};

#[cfg(feature = "persistent")]
pub use self::backing_vec::Persistent;

#[cfg(test)]
mod tests;

/// This trait is implemented by any type that can serve as a type
/// variable. We call such variables *unification keys*. For example,
/// this trait is implemented by `IntVid`, which represents integral
/// variables.
///
/// Each key type has an associated value type `V`. For example, for
/// `IntVid`, this is `Option<IntVarValue>`, representing some
/// (possibly not yet known) sort of integer.
///
/// Clients are expected to provide implementations of this trait; you
/// can see some examples in the `test` module.
pub trait UnifyKey: Copy + Clone + Debug + PartialEq {
    type Value: UnifyValue;

    fn index(&self) -> u32;

    fn from_index(u: u32) -> Self;

    fn tag() -> &'static str;

    /// You should return first the key that should be used as root,
    /// then the other key (that will then point to the new root).
    ///
    /// NB. The only reason to implement this method is if you want to
    /// control what value is returned from `find()`. In general, it
    /// is better to let the unification table determine the root,
    /// since overriding the rank can cause execution time to increase
    /// dramatically.
    #[allow(unused_variables)]
    fn order_roots(
        a: Self,
        a_value: &Self::Value,
        b: Self,
        b_value: &Self::Value,
    ) -> Option<(Self, Self)> {
        None
    }
}

/// Trait implemented for **values** associated with a unification
/// key. This trait defines how to merge the values from two keys that
/// are unioned together. This merging can be fallible. If you attempt
/// to union two keys whose values cannot be merged, then the error is
/// propagated up and the two keys are not unioned.
///
/// This crate provides implementations of `UnifyValue` for `()`
/// (which is infallible) and `Option<T>` (where `T: UnifyValue`). The
/// option implementation merges two sum-values using the `UnifyValue`
/// implementation of `T`.
///
/// See also `EqUnifyValue`, which is a convenience trait for cases
/// where the "merge" operation succeeds only if the two values are
/// equal.
pub trait UnifyValue: Clone + Debug {
    /// Defines the type to return when merging of two values fails.
    /// If merging is infallible, use the special struct `NoError`
    /// found in this crate, which unlocks various more convenient
    /// methods on the unification table.
    type Error;

    /// Given two values, produce a new value that combines them.
    /// If that is not possible, produce an error.
    fn unify_values(value1: &Self, value2: &Self) -> Result<Self, Self::Error>;
}

/// A convenient helper for unification values which must be equal or
/// else an error occurs. For example, if you are unifying types in a
/// simple functional language, this may be appropriate, since (e.g.)
/// you can't unify a type variable bound to `int` with one bound to
/// `float` (but you can unify two type variables both bound to
/// `int`).
///
/// Any type which implements `EqUnifyValue` automatially implements
/// `UnifyValue`; if the two values are equal, merging is permitted.
/// Otherwise, the error `(v1, v2)` is returned, where `v1` and `v2`
/// are the two unequal values.
pub trait EqUnifyValue: Eq + Clone + Debug {}

impl<T: EqUnifyValue> UnifyValue for T {
    type Error = (T, T);

    fn unify_values(value1: &Self, value2: &Self) -> Result<Self, Self::Error> {
        if value1 == value2 {
            Ok(value1.clone())
        } else {
            Err((value1.clone(), value2.clone()))
        }
    }
}

/// A struct which can never be instantiated. Used
/// for the error type for infallible cases.
#[derive(Debug)]
pub struct NoError {
    _dummy: (),
}

/// Value of a unification key. We implement Tarjan's union-find
/// algorithm: when two keys are unified, one of them is converted
/// into a "redirect" pointing at the other. These redirects form a
/// DAG: the roots of the DAG (nodes that are not redirected) are each
/// associated with a value of type `V` and a rank. The rank is used
/// to keep the DAG relatively balanced, which helps keep the running
/// time of the algorithm under control. For more information, see
/// <http://en.wikipedia.org/wiki/Disjoint-set_data_structure>.
#[derive(PartialEq, Clone, Debug)]
pub struct VarValue<K: UnifyKey> {
    parent: K,       // if equal to self, this is a root
    value: K::Value, // value assigned (only relevant to root)
    rank: u32,       // max depth (only relevant to root)
}

/// Table of unification keys and their values. You must define a key type K
/// that implements the `UnifyKey` trait. Unification tables can be used in two-modes:
///
/// - in-place (`UnificationTable<InPlace<K>>` or `InPlaceUnificationTable<K>`):
///   - This is the standard mutable mode, where the array is modified
///     in place.
///   - To do backtracking, you can employ the `snapshot` and `rollback_to`
///     methods.
/// - persistent (`UnificationTable<Persistent<K>>` or `PersistentUnificationTable<K>`):
///   - In this mode, we use a persistent vector to store the data, so that
///     cloning the table is an O(1) operation.
///   - This implies that ordinary operations are quite a bit slower though.
///   - Requires the `persistent` feature be selected in your Cargo.toml file.
#[derive(Clone, Debug, Default)]
pub struct UnificationTable<S: UnificationStoreBase> {
    /// Indicates the current value of each key.
    values: S,
}

pub type UnificationStorage<K> = Vec<VarValue<K>>;
pub type UnificationTableStorage<K> = UnificationTable<InPlace<K, UnificationStorage<K>, ()>>;

/// A unification table that uses an "in-place" vector.
#[allow(type_alias_bounds)]
pub type InPlaceUnificationTable<
    K: UnifyKey,
    V: sv::VecLike<Delegate<K>> = Vec<VarValue<K>>,
    L = VecLog<UndoLog<Delegate<K>>>,
> = UnificationTable<InPlace<K, V, L>>;

/// A unification table that uses a "persistent" vector.
#[cfg(feature = "persistent")]
#[allow(type_alias_bounds)]
pub type PersistentUnificationTable<K: UnifyKey> = UnificationTable<Persistent<K>>;

/// At any time, users may snapshot a unification table.  The changes
/// made during the snapshot may either be *committed* or *rolled back*.
pub struct Snapshot<S: UnificationStore> {
    // Link snapshot to the unification store `S` of the table.
    marker: marker::PhantomData<S>,
    snapshot: S::Snapshot,
}

impl<K: UnifyKey> VarValue<K> {
    fn new_var(key: K, value: K::Value) -> VarValue<K> {
        VarValue::new(key, value, 0)
    }

    fn new(parent: K, value: K::Value, rank: u32) -> VarValue<K> {
        VarValue {
            parent: parent, // this is a root
            value: value,
            rank: rank,
        }
    }

    fn redirect(&mut self, to: K) {
        self.parent = to;
    }

    fn root(&mut self, rank: u32, value: K::Value) {
        self.rank = rank;
        self.value = value;
    }
}

impl<K> UnificationTableStorage<K>
where
    K: UnifyKey,
{
    /// Creates a `UnificationTable` using an external `undo_log`, allowing mutating methods to be
    /// called if `L` does not implement `UndoLogs`
    pub fn with_log<'a, L>(
        &'a mut self,
        undo_log: L,
    ) -> UnificationTable<InPlace<K, &'a mut UnificationStorage<K>, L>>
    where
        L: UndoLogs<sv::UndoLog<Delegate<K>>>,
    {
        UnificationTable {
            values: InPlace {
                values: self.values.values.with_log(undo_log),
            },
        }
    }
}

// We can't use V:LatticeValue, much as I would like to,
// because frequently the pattern is that V=Option<U> for some
// other type parameter U, and we have no way to say
// Option<U>:LatticeValue.

impl<S: UnificationStoreBase + Default> UnificationTable<S> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<S: UnificationStore> UnificationTable<S> {
    /// Starts a new snapshot. Each snapshot must be either
    /// rolled back or committed in a "LIFO" (stack) order.
    pub fn snapshot(&mut self) -> Snapshot<S> {
        Snapshot {
            marker: marker::PhantomData::<S>,
            snapshot: self.values.start_snapshot(),
        }
    }

    /// Reverses all changes since the last snapshot. Also
    /// removes any keys that have been created since then.
    pub fn rollback_to(&mut self, snapshot: Snapshot<S>) {
        debug!("{}: rollback_to()", S::tag());
        self.values.rollback_to(snapshot.snapshot);
    }

    /// Commits all changes since the last snapshot. Of course, they
    /// can still be undone if there is a snapshot further out.
    pub fn commit(&mut self, snapshot: Snapshot<S>) {
        debug!("{}: commit()", S::tag());
        self.values.commit(snapshot.snapshot);
    }

    /// Returns the keys of all variables created since the `snapshot`.
    pub fn vars_since_snapshot(&self, snapshot: &Snapshot<S>) -> Range<S::Key> {
        let range = self.values.values_since_snapshot(&snapshot.snapshot);
        S::Key::from_index(range.start as u32)..S::Key::from_index(range.end as u32)
    }
}

impl<S: UnificationStoreBase> UnificationTable<S> {
    /// Returns the number of keys created so far.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Obtains the current value for a particular key.
    /// Not for end-users; they can use `probe_value`.
    fn value(&self, key: S::Key) -> &VarValue<S::Key> {
        &self.values[key.index() as usize]
    }
}

impl<S: UnificationStoreMut> UnificationTable<S> {
    /// Creates a fresh key with the given value.
    pub fn new_key(&mut self, value: S::Value) -> S::Key {
        let len = self.values.len();
        let key: S::Key = UnifyKey::from_index(len as u32);
        self.values.push(VarValue::new_var(key, value));
        debug!("{}: created new key: {:?}", S::tag(), key);
        key
    }

    /// Reserve memory for `num_new_keys` to be created. Does not
    /// actually create the new keys; you must then invoke `new_key`.
    pub fn reserve(&mut self, num_new_keys: usize) {
        self.values.reserve(num_new_keys);
    }

    /// Clears all unifications that have been performed, resetting to
    /// the initial state. The values of each variable are given by
    /// the closure.
    pub fn reset_unifications(&mut self, mut value: impl FnMut(S::Key) -> S::Value) {
        self.values.reset_unifications(|i| {
            let key = UnifyKey::from_index(i as u32);
            let value = value(key);
            VarValue::new_var(key, value)
        });
    }

    /// Find the root node for `vid`. This uses the standard
    /// union-find algorithm with path compression:
    /// <http://en.wikipedia.org/wiki/Disjoint-set_data_structure>.
    ///
    /// NB. This is a building-block operation and you would probably
    /// prefer to call `probe` below.
    ///
    /// This is an always-inlined version of this function for the hot
    /// callsites. `uninlined_get_root_key` is the never-inlined version.
    #[inline(always)]
    fn inlined_get_root_key(&mut self, vid: S::Key) -> S::Key {
        let v = self.value(vid);
        if v.parent == vid {
            return vid;
        }

        let redirect = v.parent;
        let root_key: S::Key = self.uninlined_get_root_key(redirect);
        if root_key != redirect {
            // Path compression
            self.update_value(vid, |value| value.parent = root_key);
        }

        root_key
    }

    // This is a never-inlined version of this function for cold callsites.
    // 'inlined_get_root_key` is the always-inlined version.
    #[inline(never)]
    fn uninlined_get_root_key(&mut self, vid: S::Key) -> S::Key {
        self.inlined_get_root_key(vid)
    }

    fn update_value<OP>(&mut self, key: S::Key, op: OP)
    where
        OP: FnOnce(&mut VarValue<S::Key>),
    {
        self.values.update(key.index() as usize, op);
        debug!("Updated variable {:?} to {:?}", key, self.value(key));
    }

    /// Either redirects `node_a` to `node_b` or vice versa, depending
    /// on the relative rank. The value associated with the new root
    /// will be `new_value`.
    ///
    /// NB: This is the "union" operation of "union-find". It is
    /// really more of a building block. If the values associated with
    /// your key are non-trivial, you would probably prefer to call
    /// `unify_var_var` below.
    fn unify_roots(&mut self, key_a: S::Key, key_b: S::Key, new_value: S::Value) {
        debug!("unify(key_a={:?}, key_b={:?})", key_a, key_b);

        let rank_a = self.value(key_a).rank;
        let rank_b = self.value(key_b).rank;
        if let Some((new_root, redirected)) = S::Key::order_roots(
            key_a,
            &self.value(key_a).value,
            key_b,
            &self.value(key_b).value,
        ) {
            // compute the new rank for the new root that they chose;
            // this may not be the optimal choice.
            let new_rank = if new_root == key_a {
                debug_assert!(redirected == key_b);
                if rank_a > rank_b {
                    rank_a
                } else {
                    rank_b + 1
                }
            } else {
                debug_assert!(new_root == key_b);
                debug_assert!(redirected == key_a);
                if rank_b > rank_a {
                    rank_b
                } else {
                    rank_a + 1
                }
            };
            self.redirect_root(new_rank, redirected, new_root, new_value);
        } else if rank_a > rank_b {
            // a has greater rank, so a should become b's parent,
            // i.e., b should redirect to a.
            self.redirect_root(rank_a, key_b, key_a, new_value);
        } else if rank_a < rank_b {
            // b has greater rank, so a should redirect to b.
            self.redirect_root(rank_b, key_a, key_b, new_value);
        } else {
            // If equal, redirect one to the other and increment the
            // other's rank.
            self.redirect_root(rank_a + 1, key_a, key_b, new_value);
        }
    }

    /// Internal method to redirect `old_root_key` (which is currently
    /// a root) to a child of `new_root_key` (which will remain a
    /// root). The rank and value of `new_root_key` will be updated to
    /// `new_rank` and `new_value` respectively.
    fn redirect_root(
        &mut self,
        new_rank: u32,
        old_root_key: S::Key,
        new_root_key: S::Key,
        new_value: S::Value,
    ) {
        self.update_value(old_root_key, |old_root_value| {
            old_root_value.redirect(new_root_key);
        });
        self.update_value(new_root_key, |new_root_value| {
            new_root_value.root(new_rank, new_value);
        });
    }
}

/// ////////////////////////////////////////////////////////////////////////
/// Public API

impl<S, K, V> UnificationTable<S>
where
    S: UnificationStoreBase<Key = K, Value = V>,
    K: UnifyKey<Value = V>,
    V: UnifyValue,
{
    /// Obtains current value for key without any pointer chasing; may return `None` if key has been union'd.
    #[inline]
    pub fn try_probe_value<'a, K1>(&'a self, id: K1) -> Option<&'a V>
        where
            K1: Into<K>,
            K: 'a,
    {
        let id = id.into();
        let v = self.value(id);
        if v.parent == id {
            return Some(&v.value);
        }
        None
    }
}

impl<S, K, V> UnificationTable<S>
where
    S: UnificationStoreMut<Key = K, Value = V>,
    K: UnifyKey<Value = V>,
    V: UnifyValue,
{
    /// Unions two keys without the possibility of failure; only
    /// applicable when unify values use `NoError` as their error
    /// type.
    pub fn union<K1, K2>(&mut self, a_id: K1, b_id: K2)
    where
        K1: Into<K>,
        K2: Into<K>,
        V: UnifyValue<Error = NoError>,
    {
        self.unify_var_var(a_id, b_id).unwrap();
    }

    /// Unions a key and a value without the possibility of failure;
    /// only applicable when unify values use `NoError` as their error
    /// type.
    pub fn union_value<K1>(&mut self, id: K1, value: V)
    where
        K1: Into<K>,
        V: UnifyValue<Error = NoError>,
    {
        self.unify_var_value(id, value).unwrap();
    }

    /// Given two keys, indicates whether they have been unioned together.
    pub fn unioned<K1, K2>(&mut self, a_id: K1, b_id: K2) -> bool
    where
        K1: Into<K>,
        K2: Into<K>,
    {
        self.find(a_id) == self.find(b_id)
    }

    /// Given a key, returns the (current) root key.
    pub fn find<K1>(&mut self, id: K1) -> K
    where
        K1: Into<K>,
    {
        let id = id.into();
        self.uninlined_get_root_key(id)
    }

    /// Unions together two variables, merging their values. If
    /// merging the values fails, the error is propagated and this
    /// method has no effect.
    pub fn unify_var_var<K1, K2>(&mut self, a_id: K1, b_id: K2) -> Result<(), V::Error>
    where
        K1: Into<K>,
        K2: Into<K>,
    {
        let a_id = a_id.into();
        let b_id = b_id.into();

        let root_a = self.uninlined_get_root_key(a_id);
        let root_b = self.uninlined_get_root_key(b_id);

        if root_a == root_b {
            return Ok(());
        }

        let combined = V::unify_values(&self.value(root_a).value, &self.value(root_b).value)?;

        Ok(self.unify_roots(root_a, root_b, combined))
    }

    /// Sets the value of the key `a_id` to `b`, attempting to merge
    /// with the previous value.
    pub fn unify_var_value<K1>(&mut self, a_id: K1, b: V) -> Result<(), V::Error>
    where
        K1: Into<K>,
    {
        let a_id = a_id.into();
        let root_a = self.uninlined_get_root_key(a_id);
        let value = V::unify_values(&self.value(root_a).value, &b)?;
        self.update_value(root_a, |node| node.value = value);
        Ok(())
    }

    /// Returns the current value for the given key. If the key has
    /// been union'd, this will give the value from the current root.
    pub fn probe_value<K1>(&mut self, id: K1) -> V
    where
        K1: Into<K>,
    {
        self.inlined_probe_value(id)
    }

    // An always-inlined version of `probe_value`, for hot callsites.
    #[inline(always)]
    pub fn inlined_probe_value<K1>(&mut self, id: K1) -> V
    where
        K1: Into<K>,
    {
        let id = id.into();
        let id = self.inlined_get_root_key(id);
        self.value(id).value.clone()
    }
}

///////////////////////////////////////////////////////////////////////////

impl UnifyValue for () {
    type Error = NoError;

    fn unify_values(_: &(), _: &()) -> Result<(), NoError> {
        Ok(())
    }
}

impl<V: UnifyValue> UnifyValue for Option<V> {
    type Error = V::Error;

    fn unify_values(a: &Option<V>, b: &Option<V>) -> Result<Self, V::Error> {
        match (a, b) {
            (&None, &None) => Ok(None),
            (&Some(ref v), &None) | (&None, &Some(ref v)) => Ok(Some(v.clone())),
            (&Some(ref a), &Some(ref b)) => match V::unify_values(a, b) {
                Ok(v) => Ok(Some(v)),
                Err(err) => Err(err),
            },
        }
    }
}
