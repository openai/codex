#[cfg(feature = "persistent")]
use dogged::DVec;
use snapshot_vec as sv;
use std::marker::PhantomData;
use std::ops::{self, Range};

use undo_log::{Rollback, Snapshots, UndoLogs, VecLog};

use super::{UnifyKey, UnifyValue, VarValue};

#[allow(dead_code)] // rustc BUG
#[allow(type_alias_bounds)]
type Key<S: UnificationStoreBase> = <S as UnificationStoreBase>::Key;

/// Largely internal trait implemented by the unification table
/// backing store types. The most common such type is `InPlace`,
/// which indicates a standard, mutable unification table.
pub trait UnificationStoreBase: ops::Index<usize, Output = VarValue<Key<Self>>> {
    type Key: UnifyKey<Value = Self::Value>;
    type Value: UnifyValue;

    fn len(&self) -> usize;

    fn tag() -> &'static str {
        Self::Key::tag()
    }
}

pub trait UnificationStoreMut: UnificationStoreBase {
    fn reset_unifications(&mut self, value: impl FnMut(u32) -> VarValue<Self::Key>);

    fn push(&mut self, value: VarValue<Self::Key>);

    fn reserve(&mut self, num_new_values: usize);

    fn update<F>(&mut self, index: usize, op: F)
    where
        F: FnOnce(&mut VarValue<Self::Key>);
}

pub trait UnificationStore: UnificationStoreMut {
    type Snapshot;

    fn start_snapshot(&mut self) -> Self::Snapshot;

    fn rollback_to(&mut self, snapshot: Self::Snapshot);

    fn commit(&mut self, snapshot: Self::Snapshot);

    fn values_since_snapshot(&self, snapshot: &Self::Snapshot) -> Range<usize>;
}

/// Backing store for an in-place unification table.
/// Not typically used directly.
#[derive(Clone, Debug)]
pub struct InPlace<
    K: UnifyKey,
    V: sv::VecLike<Delegate<K>> = Vec<VarValue<K>>,
    L = VecLog<sv::UndoLog<Delegate<K>>>,
> {
    pub(crate) values: sv::SnapshotVec<Delegate<K>, V, L>,
}

// HACK(eddyb) manual impl avoids `Default` bound on `K`.
impl<K: UnifyKey, V: sv::VecLike<Delegate<K>> + Default, L: Default> Default for InPlace<K, V, L> {
    fn default() -> Self {
        InPlace {
            values: sv::SnapshotVec::new(),
        }
    }
}

impl<K, V, L> UnificationStoreBase for InPlace<K, V, L>
where
    K: UnifyKey,
    V: sv::VecLike<Delegate<K>>,
{
    type Key = K;
    type Value = K::Value;

    fn len(&self) -> usize {
        self.values.len()
    }
}

impl<K, V, L> UnificationStoreMut for InPlace<K, V, L>
where
    K: UnifyKey,
    V: sv::VecLike<Delegate<K>>,
    L: UndoLogs<sv::UndoLog<Delegate<K>>>,
{
    #[inline]
    fn reset_unifications(&mut self, mut value: impl FnMut(u32) -> VarValue<Self::Key>) {
        self.values.set_all(|i| value(i as u32));
    }

    #[inline]
    fn push(&mut self, value: VarValue<Self::Key>) {
        self.values.push(value);
    }

    #[inline]
    fn reserve(&mut self, num_new_values: usize) {
        self.values.reserve(num_new_values);
    }

    #[inline]
    fn update<F>(&mut self, index: usize, op: F)
    where
        F: FnOnce(&mut VarValue<Self::Key>),
    {
        self.values.update(index, op)
    }
}

impl<K, V, L> UnificationStore for InPlace<K, V, L>
where
    K: UnifyKey,
    V: sv::VecLike<Delegate<K>>,
    L: Snapshots<sv::UndoLog<Delegate<K>>>,
{
    type Snapshot = sv::Snapshot<L::Snapshot>;

    #[inline]
    fn start_snapshot(&mut self) -> Self::Snapshot {
        self.values.start_snapshot()
    }

    #[inline]
    fn rollback_to(&mut self, snapshot: Self::Snapshot) {
        self.values.rollback_to(snapshot);
    }

    #[inline]
    fn commit(&mut self, snapshot: Self::Snapshot) {
        self.values.commit(snapshot);
    }

    #[inline]
    fn values_since_snapshot(&self, snapshot: &Self::Snapshot) -> Range<usize> {
        snapshot.value_count..self.len()
    }
}

impl<K, V, L> ops::Index<usize> for InPlace<K, V, L>
where
    V: sv::VecLike<Delegate<K>>,
    K: UnifyKey,
{
    type Output = VarValue<K>;
    fn index(&self, index: usize) -> &VarValue<K> {
        &self.values[index]
    }
}

#[doc(hidden)]
#[derive(Copy, Clone, Debug)]
pub struct Delegate<K>(PhantomData<K>);

impl<K: UnifyKey> sv::SnapshotVecDelegate for Delegate<K> {
    type Value = VarValue<K>;
    type Undo = ();

    fn reverse(_: &mut Vec<VarValue<K>>, _: ()) {}
}

impl<K: UnifyKey> Rollback<sv::UndoLog<Delegate<K>>> for super::UnificationTableStorage<K> {
    fn reverse(&mut self, undo: sv::UndoLog<Delegate<K>>) {
        self.values.values.reverse(undo);
    }
}

#[cfg(feature = "persistent")]
#[derive(Clone, Debug)]
pub struct Persistent<K: UnifyKey> {
    values: DVec<VarValue<K>>,
}

// HACK(eddyb) manual impl avoids `Default` bound on `K`.
#[cfg(feature = "persistent")]
impl<K: UnifyKey> Default for Persistent<K> {
    fn default() -> Self {
        Persistent {
            values: DVec::new(),
        }
    }
}

#[cfg(feature = "persistent")]
impl<K: UnifyKey> UnificationStoreBase for Persistent<K> {
    type Key = K;
    type Value = K::Value;

    fn len(&self) -> usize {
        self.values.len()
    }
}

#[cfg(feature = "persistent")]
impl<K: UnifyKey> UnificationStoreMut for Persistent<K> {
    #[inline]
    fn reset_unifications(&mut self, mut value: impl FnMut(u32) -> VarValue<Self::Key>) {
        // Without extending dogged, there isn't obviously a more
        // efficient way to do this. But it's pretty dumb. Maybe
        // dogged needs a `map`.
        for i in 0..self.values.len() {
            self.values[i] = value(i as u32);
        }
    }

    #[inline]
    fn push(&mut self, value: VarValue<Self::Key>) {
        self.values.push(value);
    }

    #[inline]
    fn reserve(&mut self, _num_new_values: usize) {
        // not obviously relevant to DVec.
    }

    #[inline]
    fn update<F>(&mut self, index: usize, op: F)
    where
        F: FnOnce(&mut VarValue<Self::Key>),
    {
        let p = &mut self.values[index];
        op(p);
    }
}

#[cfg(feature = "persistent")]
impl<K: UnifyKey> UnificationStore for Persistent<K> {
    type Snapshot = Self;

    #[inline]
    fn start_snapshot(&mut self) -> Self::Snapshot {
        self.clone()
    }

    #[inline]
    fn rollback_to(&mut self, snapshot: Self::Snapshot) {
        *self = snapshot;
    }

    #[inline]
    fn commit(&mut self, _snapshot: Self::Snapshot) {}

    #[inline]
    fn values_since_snapshot(&self, snapshot: &Self::Snapshot) -> Range<usize> {
        snapshot.len()..self.len()
    }
}

#[cfg(feature = "persistent")]
impl<K> ops::Index<usize> for Persistent<K>
where
    K: UnifyKey,
{
    type Output = VarValue<K>;
    fn index(&self, index: usize) -> &VarValue<K> {
        &self.values[index]
    }
}
