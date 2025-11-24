use ena::unify::{UnifyKey, UnifyValue};
use crate::lr1::lane_table::table::context_set::{ContextSet, OverlappingLookahead};

/// The unification key for a set of states in the lane table
/// algorithm.  Each set of states is associated with a
/// `ContextSet`. When two sets of states are merged, their conflict
/// sets are merged as well; this will fail if that would produce an
/// overlapping conflict set.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StateSet {
    index: u32,
}

impl UnifyKey for StateSet {
    type Value = ContextSet;

    fn index(&self) -> u32 {
        self.index
    }

    fn from_index(u: u32) -> Self {
        StateSet { index: u }
    }

    fn tag() -> &'static str {
        "StateSet"
    }
}

// FIXME: The `ena` interface is really designed around `UnifyValue`
// being cheaply cloneable; we should either refactor `ena` a bit or
// find some other way to associate a `ContextSet` with a state set
// (for example, we could have each state set be associated with an
// index that maps to a `ContextSet`), and do the merging ourselves.
// But this is easier for now, and cloning a `ContextSet` isn't THAT
// expensive, right? :)
impl UnifyValue for ContextSet {
    type Error = (Self, Self);

    fn unify_values(value1: &Self, value2: &Self) -> Result<Self, (Self, Self)> {
        match ContextSet::union(value1, value2) {
            Ok(v) => Ok(v),
            Err(OverlappingLookahead) => Err((value1.clone(), value2.clone())),
        }
    }
}
