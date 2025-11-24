#[macro_use]
extern crate log;
extern crate ena;

use ena::{
    snapshot_vec as sv,
    undo_log::{Rollback, Snapshots, UndoLogs},
    unify::{self as ut, EqUnifyValue, UnifyKey},
};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
struct IntKey(u32);

impl UnifyKey for IntKey {
    type Value = Option<IntKey>;
    fn index(&self) -> u32 {
        self.0
    }
    fn from_index(u: u32) -> IntKey {
        IntKey(u)
    }
    fn tag() -> &'static str {
        "IntKey"
    }
}

impl EqUnifyValue for IntKey {}

enum UndoLog {
    EqRelation(sv::UndoLog<ut::Delegate<IntKey>>),
    Values(sv::UndoLog<i32>),
}

impl From<sv::UndoLog<ut::Delegate<IntKey>>> for UndoLog {
    fn from(l: sv::UndoLog<ut::Delegate<IntKey>>) -> Self {
        UndoLog::EqRelation(l)
    }
}

impl From<sv::UndoLog<i32>> for UndoLog {
    fn from(l: sv::UndoLog<i32>) -> Self {
        UndoLog::Values(l)
    }
}

impl Rollback<UndoLog> for TypeVariableStorage {
    fn reverse(&mut self, undo: UndoLog) {
        match undo {
            UndoLog::EqRelation(undo) => self.eq_relations.reverse(undo),
            UndoLog::Values(undo) => self.values.reverse(undo),
        }
    }
}

#[derive(Default)]
struct TypeVariableStorage {
    values: sv::SnapshotVecStorage<i32>,

    eq_relations: ut::UnificationTableStorage<IntKey>,
}

impl TypeVariableStorage {
    fn with_log<'a>(&'a mut self, undo_log: &'a mut TypeVariableUndoLogs) -> TypeVariableTable<'a> {
        TypeVariableTable {
            storage: self,
            undo_log,
        }
    }

    fn len(&mut self) -> usize {
        assert_eq!(self.values.len(), self.eq_relations.len());
        self.values.len()
    }
}

struct TypeVariableTable<'a> {
    storage: &'a mut TypeVariableStorage,

    undo_log: &'a mut TypeVariableUndoLogs,
}

impl TypeVariableTable<'_> {
    fn new(&mut self, i: i32) -> IntKey {
        self.storage.values.with_log(&mut self.undo_log).push(i);
        self.storage
            .eq_relations
            .with_log(&mut self.undo_log)
            .new_key(None)
    }
}

struct Snapshot {
    undo_len: usize,
}

struct TypeVariableUndoLogs {
    logs: Vec<UndoLog>,
    num_open_snapshots: usize,
}

impl Default for TypeVariableUndoLogs {
    fn default() -> Self {
        Self {
            logs: Default::default(),
            num_open_snapshots: Default::default(),
        }
    }
}

impl<T> UndoLogs<T> for TypeVariableUndoLogs
where
    UndoLog: From<T>,
{
    fn num_open_snapshots(&self) -> usize {
        self.num_open_snapshots
    }
    fn push(&mut self, undo: T) {
        if self.in_snapshot() {
            self.logs.push(undo.into())
        }
    }
    fn clear(&mut self) {
        self.logs.clear();
        self.num_open_snapshots = 0;
    }
    fn extend<J>(&mut self, undos: J)
    where
        Self: Sized,
        J: IntoIterator<Item = T>,
    {
        if self.in_snapshot() {
            self.logs.extend(undos.into_iter().map(UndoLog::from))
        }
    }
}

impl Snapshots<UndoLog> for TypeVariableUndoLogs {
    type Snapshot = Snapshot;
    fn actions_since_snapshot(&self, snapshot: &Self::Snapshot) -> &[UndoLog] {
        &self.logs[snapshot.undo_len..]
    }

    fn start_snapshot(&mut self) -> Self::Snapshot {
        self.num_open_snapshots += 1;
        Snapshot {
            undo_len: self.logs.len(),
        }
    }

    fn rollback_to<R>(&mut self, values: impl FnOnce() -> R, snapshot: Self::Snapshot)
    where
        R: Rollback<UndoLog>,
    {
        debug!("rollback_to({})", snapshot.undo_len);

        if self.logs.len() > snapshot.undo_len {
            let mut values = values();
            while self.logs.len() > snapshot.undo_len {
                values.reverse(self.logs.pop().unwrap());
            }
        }

        if self.num_open_snapshots == 1 {
            // The root snapshot. It's safe to clear the undo log because
            // there's no snapshot further out that we might need to roll back
            // to.
            assert!(snapshot.undo_len == 0);
            self.logs.clear();
        }

        self.num_open_snapshots -= 1;
    }

    fn commit(&mut self, snapshot: Self::Snapshot) {
        debug!("commit({})", snapshot.undo_len);

        if self.num_open_snapshots == 1 {
            // The root snapshot. It's safe to clear the undo log because
            // there's no snapshot further out that we might need to roll back
            // to.
            assert!(snapshot.undo_len == 0);
            self.logs.clear();
        }

        self.num_open_snapshots -= 1;
    }
}

/// Tests that a undo log stored externally can be used with TypeVariableTable
#[test]
fn external_undo_log() {
    let mut storage = TypeVariableStorage::default();
    let mut undo_log = TypeVariableUndoLogs::default();

    let snapshot = undo_log.start_snapshot();
    storage.with_log(&mut undo_log).new(1);
    storage.with_log(&mut undo_log).new(2);
    assert_eq!(storage.len(), 2);

    undo_log.rollback_to(|| &mut storage, snapshot);
    assert_eq!(storage.len(), 0);
}
