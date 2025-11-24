//! Module which contains the snapshot/rollback functionality of the `ena` data structures.
//!
//! For most usecases this is just an internal implementation detail. However if many `ena`
//! data structures are snapshotted simultaneously it is possible to use
//! `UnificationTableStorage`/`SnapshotVecStorage` instead and use a custom `UndoLogs<T>`
//! type capable of recording the actions of all used data structures.
//!
//! Since the `*Storage` variants do not have an undo log `with_log` must be called with the
//! unified log before any mutating actions.

/// A trait which allows undo actions (`T`) to be pushed which can be used to rollback actions at a
/// later time if needed.
///
/// The undo actions themselves are opaque to `UndoLogs`, only specified `Rollback` implementations
/// need to know what an action is and how to reverse it.
pub trait UndoLogs<T> {
    /// True if a snapshot has started, false otherwise
    fn in_snapshot(&self) -> bool {
        self.num_open_snapshots() > 0
    }

    /// How many open snapshots this undo log currently has
    fn num_open_snapshots(&self) -> usize;

    /// Pushes a new "undo item" onto the undo log. This method is invoked when some action is taken
    /// (e.g., a variable is unified). It records the info needed to reverse that action should an
    /// enclosing snapshot be rolled back.
    fn push(&mut self, undo: T);

    /// Removes all items from the undo log.
    fn clear(&mut self);

    /// Extends the undo log with many undos.
    fn extend<I>(&mut self, undos: I)
    where
        Self: Sized,
        I: IntoIterator<Item = T>,
    {
        undos.into_iter().for_each(|undo| self.push(undo));
    }
}

impl<'a, T, U> UndoLogs<T> for &'a mut U
where
    U: UndoLogs<T>,
{
    fn in_snapshot(&self) -> bool {
        U::in_snapshot(self)
    }
    fn num_open_snapshots(&self) -> usize {
        U::num_open_snapshots(self)
    }
    fn push(&mut self, undo: T) {
        U::push(self, undo)
    }
    fn clear(&mut self) {
        U::clear(self);
    }
    fn extend<I>(&mut self, undos: I)
    where
        I: IntoIterator<Item = T>,
    {
        U::extend(self, undos)
    }
}

/// A trait which extends `UndoLogs` to allow snapshots to be done at specific points. Each snapshot can then be used to
/// rollback any changes to an underlying data structures if they were not desirable.
///
/// Each snapshot must be consumed linearly with either `rollback_to` or `commit`.
pub trait Snapshots<T>: UndoLogs<T> {
    type Snapshot;

    /// Returns true if `self` has made any changes since snapshot started.
    fn has_changes(&self, snapshot: &Self::Snapshot) -> bool {
        !self.actions_since_snapshot(snapshot).is_empty()
    }

    /// Returns the slice of actions that were taken since the snapshot began.
    fn actions_since_snapshot(&self, snapshot: &Self::Snapshot) -> &[T];

    /// Starts a new snapshot. That snapshot must eventually either be committed via a call to
    /// commit or rollback via rollback_to. Snapshots can be nested (i.e., you can start a snapshot
    /// whilst another snapshot is in progress) but you must then commit or rollback the inner
    /// snapshot before attempting to commit or rollback the outer snapshot.
    fn start_snapshot(&mut self) -> Self::Snapshot;

    /// Rollback (undo) the changes made to `storage` since the snapshot.
    fn rollback_to<R>(&mut self, storage: impl FnOnce() -> R, snapshot: Self::Snapshot)
    where
        R: Rollback<T>;

    /// Commit: keep the changes that have been made since the snapshot began
    fn commit(&mut self, snapshot: Self::Snapshot);
}

impl<T, U> Snapshots<T> for &'_ mut U
where
    U: Snapshots<T>,
{
    type Snapshot = U::Snapshot;
    fn has_changes(&self, snapshot: &Self::Snapshot) -> bool {
        U::has_changes(self, snapshot)
    }
    fn actions_since_snapshot(&self, snapshot: &Self::Snapshot) -> &[T] {
        U::actions_since_snapshot(self, snapshot)
    }

    fn start_snapshot(&mut self) -> Self::Snapshot {
        U::start_snapshot(self)
    }
    fn rollback_to<R>(&mut self, storage: impl FnOnce() -> R, snapshot: Self::Snapshot)
    where
        R: Rollback<T>,
    {
        U::rollback_to(self, storage, snapshot)
    }

    fn commit(&mut self, snapshot: Self::Snapshot) {
        U::commit(self, snapshot)
    }
}

pub struct NoUndo;
impl<T> UndoLogs<T> for NoUndo {
    fn num_open_snapshots(&self) -> usize {
        0
    }
    fn push(&mut self, _undo: T) {}
    fn clear(&mut self) {}
}

/// A basic undo log.
#[derive(Clone, Debug)]
pub struct VecLog<T> {
    log: Vec<T>,
    num_open_snapshots: usize,
}

impl<T> Default for VecLog<T> {
    fn default() -> Self {
        VecLog {
            log: Vec::new(),
            num_open_snapshots: 0,
        }
    }
}

impl<T> UndoLogs<T> for VecLog<T> {
    fn num_open_snapshots(&self) -> usize {
        self.num_open_snapshots
    }
    fn push(&mut self, undo: T) {
        self.log.push(undo);
    }
    fn clear(&mut self) {
        self.log.clear();
        self.num_open_snapshots = 0;
    }
}

impl<T> Snapshots<T> for VecLog<T> {
    type Snapshot = Snapshot;

    fn has_changes(&self, snapshot: &Self::Snapshot) -> bool {
        self.log.len() > snapshot.undo_len
    }
    fn actions_since_snapshot(&self, snapshot: &Snapshot) -> &[T] {
        &self.log[snapshot.undo_len..]
    }

    fn start_snapshot(&mut self) -> Snapshot {
        self.num_open_snapshots += 1;
        Snapshot {
            undo_len: self.log.len(),
        }
    }

    fn rollback_to<R>(&mut self, values: impl FnOnce() -> R, snapshot: Snapshot)
    where
        R: Rollback<T>,
    {
        debug!("rollback_to({})", snapshot.undo_len);

        self.assert_open_snapshot(&snapshot);

        if self.log.len() > snapshot.undo_len {
            let mut values = values();
            while self.log.len() > snapshot.undo_len {
                values.reverse(self.log.pop().unwrap());
            }
        }

        self.num_open_snapshots -= 1;
    }

    fn commit(&mut self, snapshot: Snapshot) {
        debug!("commit({})", snapshot.undo_len);

        self.assert_open_snapshot(&snapshot);

        if self.num_open_snapshots == 1 {
            // The root snapshot. It's safe to clear the undo log because
            // there's no snapshot further out that we might need to roll back
            // to.
            assert!(snapshot.undo_len == 0);
            self.log.clear();
        }

        self.num_open_snapshots -= 1;
    }
}

impl<T> VecLog<T> {
    fn assert_open_snapshot(&self, snapshot: &Snapshot) {
        // Failures here may indicate a failure to follow a stack discipline.
        assert!(self.log.len() >= snapshot.undo_len);
        assert!(self.num_open_snapshots > 0);
    }
}

impl<T> std::ops::Index<usize> for VecLog<T> {
    type Output = T;
    fn index(&self, key: usize) -> &T {
        &self.log[key]
    }
}

/// A trait implemented for storage types (like `SnapshotVecStorage`) which can be rolled back using actions of type `U`.
pub trait Rollback<U> {
    fn reverse(&mut self, undo: U);
}

impl<T, U> Rollback<U> for &'_ mut T
where
    T: Rollback<U>,
{
    fn reverse(&mut self, undo: U) {
        T::reverse(self, undo)
    }
}

/// Snapshots are tokens that should be created/consumed linearly.
pub struct Snapshot {
    // Length of the undo log at the time the snapshot was taken.
    undo_len: usize,
}
