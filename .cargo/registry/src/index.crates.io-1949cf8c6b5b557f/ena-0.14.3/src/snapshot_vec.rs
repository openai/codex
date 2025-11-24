// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A utility class for implementing "snapshottable" things; a snapshottable data structure permits
//! you to take a snapshot (via `start_snapshot`) and then, after making some changes, elect either
//! to rollback to the start of the snapshot or commit those changes.
//!
//! This vector is intended to be used as part of an abstraction, not serve as a complete
//! abstraction on its own. As such, while it will roll back most changes on its own, it also
//! supports a `get_mut` operation that gives you an arbitrary mutable pointer into the vector. To
//! ensure that any changes you make this with this pointer are rolled back, you must invoke
//! `record` to record any changes you make and also supplying a delegate capable of reversing
//! those changes.

use self::UndoLog::*;

use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::ops;

use undo_log::{Rollback, Snapshots, UndoLogs, VecLog};

#[derive(Debug)]
pub enum UndoLog<D: SnapshotVecDelegate> {
    /// New variable with given index was created.
    NewElem(usize),

    /// Variable with given index was changed *from* the given value.
    SetElem(usize, D::Value),

    /// Extensible set of actions
    Other(D::Undo),
}

impl<D: SnapshotVecDelegate> Rollback<UndoLog<D>> for SnapshotVecStorage<D> {
    fn reverse(&mut self, undo: UndoLog<D>) {
        self.values.reverse(undo)
    }
}
impl<D: SnapshotVecDelegate> Rollback<UndoLog<D>> for Vec<D::Value> {
    fn reverse(&mut self, undo: UndoLog<D>) {
        match undo {
            NewElem(i) => {
                self.pop();
                assert!(Vec::len(self) == i);
            }

            SetElem(i, v) => {
                self[i] = v;
            }

            Other(u) => {
                D::reverse(self, u);
            }
        }
    }
}

pub trait VecLike<D>: AsRef<[D::Value]> + AsMut<[D::Value]> + Rollback<UndoLog<D>>
where
    D: SnapshotVecDelegate,
{
    fn push(&mut self, item: D::Value);
    fn len(&self) -> usize;
    fn reserve(&mut self, size: usize);
}

impl<D> VecLike<D> for Vec<D::Value>
where
    D: SnapshotVecDelegate,
{
    fn push(&mut self, item: D::Value) {
        Vec::push(self, item)
    }
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn reserve(&mut self, size: usize) {
        Vec::reserve(self, size)
    }
}

impl<D> VecLike<D> for &'_ mut Vec<D::Value>
where
    D: SnapshotVecDelegate,
{
    fn push(&mut self, item: D::Value) {
        Vec::push(self, item)
    }
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn reserve(&mut self, size: usize) {
        Vec::reserve(self, size)
    }
}

#[allow(type_alias_bounds)]
pub type SnapshotVecStorage<D: SnapshotVecDelegate> =
    SnapshotVec<D, Vec<<D as SnapshotVecDelegate>::Value>, ()>;

pub struct SnapshotVec<
    D: SnapshotVecDelegate,
    V: VecLike<D> = Vec<<D as SnapshotVecDelegate>::Value>,
    L = VecLog<UndoLog<D>>,
> {
    values: V,
    undo_log: L,
    _marker: PhantomData<D>,
}

impl<D, V, L> fmt::Debug for SnapshotVec<D, V, L>
where
    D: SnapshotVecDelegate,
    V: VecLike<D> + fmt::Debug,
    L: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("SnapshotVec")
            .field("values", &self.values)
            .field("undo_log", &self.undo_log)
            .finish()
    }
}

// Snapshots are tokens that should be created/consumed linearly.
pub struct Snapshot<S = ::undo_log::Snapshot> {
    pub(crate) value_count: usize,
    snapshot: S,
}

pub trait SnapshotVecDelegate {
    type Value;
    type Undo;

    fn reverse(values: &mut Vec<Self::Value>, action: Self::Undo);
}

// HACK(eddyb) manual impl avoids `Default` bound on `D`.
impl<D: SnapshotVecDelegate, V: VecLike<D> + Default, L: Default> Default for SnapshotVec<D, V, L> {
    fn default() -> Self {
        SnapshotVec {
            values: V::default(),
            undo_log: Default::default(),
            _marker: PhantomData,
        }
    }
}

impl<D: SnapshotVecDelegate, V: VecLike<D> + Default, L: Default> SnapshotVec<D, V, L> {
    /// Creates a new `SnapshotVec`. If `L` is set to `()` then most mutating functions will not
    /// be accessible without calling `with_log` and supplying a compatibly `UndoLogs` instance.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<D: SnapshotVecDelegate> SnapshotVecStorage<D> {
    /// Creates a `SnapshotVec` using the `undo_log`, allowing mutating methods to be called
    pub fn with_log<'a, L>(
        &'a mut self,
        undo_log: L,
    ) -> SnapshotVec<D, &'a mut Vec<<D as SnapshotVecDelegate>::Value>, L>
    where
        L: UndoLogs<UndoLog<D>>,
    {
        SnapshotVec {
            values: &mut self.values,
            undo_log,
            _marker: PhantomData,
        }
    }
}

impl<D: SnapshotVecDelegate, L: Default> SnapshotVec<D, Vec<D::Value>, L> {
    pub fn with_capacity(c: usize) -> Self {
        SnapshotVec {
            values: Vec::with_capacity(c),
            undo_log: Default::default(),
            _marker: PhantomData,
        }
    }
}

impl<V: VecLike<D>, D: SnapshotVecDelegate, U> SnapshotVec<D, V, U> {
    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn get(&self, index: usize) -> &D::Value {
        &self.values.as_ref()[index]
    }

    /// Returns a mutable pointer into the vec; whatever changes you make here cannot be undone
    /// automatically, so you should be sure call `record()` with some sort of suitable undo
    /// action.
    pub fn get_mut(&mut self, index: usize) -> &mut D::Value {
        &mut self.values.as_mut()[index]
    }

    /// Reserve space for new values, just like an ordinary vec.
    pub fn reserve(&mut self, additional: usize) {
        // This is not affected by snapshots or anything.
        self.values.reserve(additional);
    }
}

impl<V: VecLike<D>, D: SnapshotVecDelegate, L: UndoLogs<UndoLog<D>>> SnapshotVec<D, V, L> {
    fn in_snapshot(&self) -> bool {
        self.undo_log.in_snapshot()
    }

    pub fn record(&mut self, action: D::Undo) {
        if self.in_snapshot() {
            self.undo_log.push(Other(action));
        }
    }

    pub fn push(&mut self, elem: D::Value) -> usize {
        let len = self.values.len();
        self.values.push(elem);

        if self.in_snapshot() {
            self.undo_log.push(NewElem(len));
        }

        len
    }

    /// Updates the element at the given index. The old value will saved (and perhaps restored) if
    /// a snapshot is active.
    pub fn set(&mut self, index: usize, new_elem: D::Value) {
        let old_elem = mem::replace(&mut self.values.as_mut()[index], new_elem);
        if self.undo_log.in_snapshot() {
            self.undo_log.push(SetElem(index, old_elem));
        }
    }

    /// Updates all elements. Potentially more efficient -- but
    /// otherwise equivalent to -- invoking `set` for each element.
    pub fn set_all(&mut self, mut new_elems: impl FnMut(usize) -> D::Value) {
        if !self.undo_log.in_snapshot() {
            for (index, slot) in self.values.as_mut().iter_mut().enumerate() {
                *slot = new_elems(index);
            }
        } else {
            for i in 0..self.values.len() {
                self.set(i, new_elems(i));
            }
        }
    }

    pub fn update<OP>(&mut self, index: usize, op: OP)
    where
        OP: FnOnce(&mut D::Value),
        D::Value: Clone,
    {
        if self.undo_log.in_snapshot() {
            let old_elem = self.values.as_mut()[index].clone();
            self.undo_log.push(SetElem(index, old_elem));
        }
        op(&mut self.values.as_mut()[index]);
    }
}

impl<D, V, L> SnapshotVec<D, V, L>
where
    D: SnapshotVecDelegate,
    V: VecLike<D> + Rollback<UndoLog<D>>,
    L: Snapshots<UndoLog<D>>,
{
    pub fn start_snapshot(&mut self) -> Snapshot<L::Snapshot> {
        Snapshot {
            value_count: self.values.len(),
            snapshot: self.undo_log.start_snapshot(),
        }
    }

    pub fn actions_since_snapshot(&self, snapshot: &Snapshot<L::Snapshot>) -> &[UndoLog<D>] {
        self.undo_log.actions_since_snapshot(&snapshot.snapshot)
    }

    pub fn rollback_to(&mut self, snapshot: Snapshot<L::Snapshot>) {
        let values = &mut self.values;
        self.undo_log.rollback_to(|| values, snapshot.snapshot);
    }

    /// Commits all changes since the last snapshot. Of course, they
    /// can still be undone if there is a snapshot further out.
    pub fn commit(&mut self, snapshot: Snapshot<L::Snapshot>) {
        self.undo_log.commit(snapshot.snapshot);
    }
}

impl<D: SnapshotVecDelegate, V: VecLike<D>, L> ops::Deref for SnapshotVec<D, V, L> {
    type Target = [D::Value];
    fn deref(&self) -> &[D::Value] {
        self.values.as_ref()
    }
}

impl<D: SnapshotVecDelegate, V: VecLike<D>, L> ops::DerefMut for SnapshotVec<D, V, L> {
    fn deref_mut(&mut self) -> &mut [D::Value] {
        self.values.as_mut()
    }
}

impl<D: SnapshotVecDelegate, V: VecLike<D>, L> ops::Index<usize> for SnapshotVec<D, V, L> {
    type Output = D::Value;
    fn index(&self, index: usize) -> &D::Value {
        self.get(index)
    }
}

impl<D: SnapshotVecDelegate, V: VecLike<D>, L> ops::IndexMut<usize> for SnapshotVec<D, V, L> {
    fn index_mut(&mut self, index: usize) -> &mut D::Value {
        self.get_mut(index)
    }
}

impl<D: SnapshotVecDelegate, V: VecLike<D> + Extend<D::Value>> Extend<D::Value>
    for SnapshotVec<D, V>
{
    fn extend<T>(&mut self, iterable: T)
    where
        T: IntoIterator<Item = D::Value>,
    {
        let initial_len = self.values.len();
        self.values.extend(iterable);
        let final_len = self.values.len();

        if self.in_snapshot() {
            self.undo_log
                .extend((initial_len..final_len).map(|len| NewElem(len)));
        }
    }
}

impl<D: SnapshotVecDelegate, V, L> Clone for SnapshotVec<D, V, L>
where
    V: VecLike<D> + Clone,
    L: Clone,
{
    fn clone(&self) -> Self {
        SnapshotVec {
            values: self.values.clone(),
            undo_log: self.undo_log.clone(),
            _marker: PhantomData,
        }
    }
}

impl<D: SnapshotVecDelegate> Clone for UndoLog<D>
where
    D::Value: Clone,
    D::Undo: Clone,
{
    fn clone(&self) -> Self {
        match *self {
            NewElem(i) => NewElem(i),
            SetElem(i, ref v) => SetElem(i, v.clone()),
            Other(ref u) => Other(u.clone()),
        }
    }
}

impl SnapshotVecDelegate for i32 {
    type Value = i32;
    type Undo = ();

    fn reverse(_: &mut Vec<i32>, _: ()) {}
}

#[test]
fn basic() {
    let mut vec: SnapshotVec<i32> = SnapshotVec::default();
    assert!(!vec.in_snapshot());
    assert_eq!(vec.len(), 0);
    vec.push(22);
    vec.push(33);
    assert_eq!(vec.len(), 2);
    assert_eq!(*vec.get(0), 22);
    assert_eq!(*vec.get(1), 33);
    vec.set(1, 34);
    assert_eq!(vec.len(), 2);
    assert_eq!(*vec.get(0), 22);
    assert_eq!(*vec.get(1), 34);

    let snapshot = vec.start_snapshot();
    assert!(vec.in_snapshot());

    vec.push(44);
    vec.push(55);
    vec.set(1, 35);
    assert_eq!(vec.len(), 4);
    assert_eq!(*vec.get(0), 22);
    assert_eq!(*vec.get(1), 35);
    assert_eq!(*vec.get(2), 44);
    assert_eq!(*vec.get(3), 55);

    vec.rollback_to(snapshot);
    assert!(!vec.in_snapshot());

    assert_eq!(vec.len(), 2);
    assert_eq!(*vec.get(0), 22);
    assert_eq!(*vec.get(1), 34);
}

#[test]
#[should_panic]
fn out_of_order() {
    let mut vec: SnapshotVec<i32> = SnapshotVec::default();
    vec.push(22);
    let snapshot1 = vec.start_snapshot();
    vec.push(33);
    let snapshot2 = vec.start_snapshot();
    vec.push(44);
    vec.rollback_to(snapshot1); // bogus, but accepted
    vec.rollback_to(snapshot2); // asserts
}

#[test]
fn nested_commit_then_rollback() {
    let mut vec: SnapshotVec<i32> = SnapshotVec::default();
    vec.push(22);
    let snapshot1 = vec.start_snapshot();
    let snapshot2 = vec.start_snapshot();
    vec.set(0, 23);
    vec.commit(snapshot2);
    assert_eq!(*vec.get(0), 23);
    vec.rollback_to(snapshot1);
    assert_eq!(*vec.get(0), 22);
}
