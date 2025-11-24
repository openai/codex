//! This module contains property-based tests against the public API:
//! * API never panics.
//! * Active entries cannot be overridden until removed.
//! * The slab doesn't produce overlapping keys.
//! * The slab doesn't leave "lost" keys.
//! * `get()`, `get_owned`, and `contains()` are consistent.
//! * `RESERVED_BITS` are actually not used.
//!
//! The test is supposed to be deterministic, so it doesn't spawn real threads
//! and uses `tid::with()` to override the TID for the current thread.

use std::{ops::Range, sync::Arc};

use indexmap::IndexMap;
use proptest::prelude::*;

use crate::{tid, Config, DefaultConfig, Slab};

const THREADS: Range<usize> = 1..4;
const ACTIONS: Range<usize> = 1..1000;

#[derive(Debug, Clone)]
struct Action {
    tid: usize,
    kind: ActionKind,
}

#[derive(Debug, Clone)]
enum ActionKind {
    Insert,
    VacantEntry,
    RemoveRandom(usize),   // key
    RemoveExistent(usize), // seed
    TakeRandom(usize),     // key
    TakeExistent(usize),   // seed
    GetRandom(usize),      // key
    GetExistent(usize),    // seed
}

prop_compose! {
    fn action_strategy()(tid in THREADS, kind in action_kind_strategy()) -> Action {
        Action { tid, kind }
    }
}

fn action_kind_strategy() -> impl Strategy<Value = ActionKind> {
    prop_oneof![
        1 => Just(ActionKind::Insert),
        1 => Just(ActionKind::VacantEntry),
        1 => prop::num::usize::ANY.prop_map(ActionKind::RemoveRandom),
        1 => prop::num::usize::ANY.prop_map(ActionKind::RemoveExistent),
        1 => prop::num::usize::ANY.prop_map(ActionKind::TakeRandom),
        1 => prop::num::usize::ANY.prop_map(ActionKind::TakeExistent),
        // Produce `GetRandom` and `GetExistent` more often.
        5 => prop::num::usize::ANY.prop_map(ActionKind::GetRandom),
        5 => prop::num::usize::ANY.prop_map(ActionKind::GetExistent),
    ]
}

/// Stores active entries (added and not yet removed).
#[derive(Default)]
struct Active {
    // Use `IndexMap` to preserve determinism.
    map: IndexMap<usize, u32>,
    prev_value: u32,
}

impl Active {
    fn next_value(&mut self) -> u32 {
        self.prev_value += 1;
        self.prev_value
    }

    fn get(&self, key: usize) -> Option<u32> {
        self.map.get(&key).copied()
    }

    fn get_any(&self, seed: usize) -> Option<(usize, u32)> {
        if self.map.is_empty() {
            return None;
        }

        let index = seed % self.map.len();
        self.map.get_index(index).map(|(k, v)| (*k, *v))
    }

    fn insert(&mut self, key: usize, value: u32) {
        assert_eq!(
            self.map.insert(key, value),
            None,
            "keys of active entries must be unique"
        );
    }

    fn remove(&mut self, key: usize) -> Option<u32> {
        self.map.swap_remove(&key)
    }

    fn remove_any(&mut self, seed: usize) -> Option<(usize, u32)> {
        if self.map.is_empty() {
            return None;
        }

        let index = seed % self.map.len();
        self.map.swap_remove_index(index)
    }

    fn drain(&mut self) -> impl Iterator<Item = (usize, u32)> + '_ {
        self.map.drain(..)
    }
}

fn used_bits<C: Config>(key: usize) -> usize {
    assert_eq!(
        C::RESERVED_BITS + Slab::<u32, C>::USED_BITS,
        std::mem::size_of::<usize>() * 8
    );
    key & ((!0) >> C::RESERVED_BITS)
}

fn apply_action<C: Config>(
    slab: &Arc<Slab<u32, C>>,
    active: &mut Active,
    action: ActionKind,
) -> Result<(), TestCaseError> {
    match action {
        ActionKind::Insert => {
            let value = active.next_value();
            let key = slab.insert(value).expect("unexpectedly exhausted slab");
            prop_assert_eq!(used_bits::<C>(key), key);
            active.insert(key, value);
        }
        ActionKind::VacantEntry => {
            let value = active.next_value();
            let entry = slab.vacant_entry().expect("unexpectedly exhausted slab");
            let key = entry.key();
            prop_assert_eq!(used_bits::<C>(key), key);
            entry.insert(value);
            active.insert(key, value);
        }
        ActionKind::RemoveRandom(key) => {
            let used_key = used_bits::<C>(key);
            prop_assert_eq!(slab.get(key).map(|e| *e), slab.get(used_key).map(|e| *e));
            prop_assert_eq!(slab.remove(key), active.remove(used_key).is_some());
        }
        ActionKind::RemoveExistent(seed) => {
            if let Some((key, _value)) = active.remove_any(seed) {
                prop_assert!(slab.contains(key));
                prop_assert!(slab.remove(key));
            }
        }
        ActionKind::TakeRandom(key) => {
            let used_key = used_bits::<C>(key);
            prop_assert_eq!(slab.get(key).map(|e| *e), slab.get(used_key).map(|e| *e));
            prop_assert_eq!(slab.take(key), active.remove(used_key));
        }
        ActionKind::TakeExistent(seed) => {
            if let Some((key, value)) = active.remove_any(seed) {
                prop_assert!(slab.contains(key));
                prop_assert_eq!(slab.take(key), Some(value));
            }
        }
        ActionKind::GetRandom(key) => {
            let used_key = used_bits::<C>(key);
            prop_assert_eq!(slab.get(key).map(|e| *e), slab.get(used_key).map(|e| *e));
            prop_assert_eq!(slab.get(key).map(|e| *e), active.get(used_key));
            prop_assert_eq!(
                slab.clone().get_owned(key).map(|e| *e),
                active.get(used_key)
            );
        }
        ActionKind::GetExistent(seed) => {
            if let Some((key, value)) = active.get_any(seed) {
                prop_assert!(slab.contains(key));
                prop_assert_eq!(slab.get(key).map(|e| *e), Some(value));
                prop_assert_eq!(slab.clone().get_owned(key).map(|e| *e), Some(value));
            }
        }
    }

    Ok(())
}

fn run<C: Config>(actions: Vec<Action>) -> Result<(), TestCaseError> {
    let mut slab = Arc::new(Slab::new_with_config::<C>());
    let mut active = Active::default();

    // Apply all actions.
    for action in actions {
        // Override the TID for the current thread instead of using multiple real threads
        // to preserve determinism. We're not checking concurrency issues here, they should be
        // covered by loom tests anyway. Thus, it's fine to run all actions consequently.
        tid::with(action.tid, || {
            apply_action::<C>(&slab, &mut active, action.kind)
        })?;
    }

    // Ensure the slab contains all remaining entries.
    let mut expected_values = Vec::new();
    for (key, value) in active.drain() {
        prop_assert!(slab.contains(key));
        prop_assert_eq!(slab.get(key).map(|e| *e), Some(value));
        prop_assert_eq!(slab.clone().get_owned(key).map(|e| *e), Some(value));
        expected_values.push(value);
    }
    expected_values.sort();

    // Ensure `unique_iter()` returns all remaining entries.
    let slab = Arc::get_mut(&mut slab).unwrap();
    let mut actual_values = slab.unique_iter().copied().collect::<Vec<_>>();
    actual_values.sort();
    prop_assert_eq!(actual_values, expected_values);

    Ok(())
}

proptest! {
    #[test]
    fn default_config(actions in prop::collection::vec(action_strategy(), ACTIONS)) {
        run::<DefaultConfig>(actions)?;
    }

    #[test]
    fn custom_config(actions in prop::collection::vec(action_strategy(), ACTIONS)) {
        run::<CustomConfig>(actions)?;
    }
}

struct CustomConfig;

#[cfg(target_pointer_width = "64")]
impl Config for CustomConfig {
    const INITIAL_PAGE_SIZE: usize = 32;
    const MAX_PAGES: usize = 15;
    const MAX_THREADS: usize = 256;
    const RESERVED_BITS: usize = 24;
}
#[cfg(target_pointer_width = "32")]
impl Config for CustomConfig {
    const INITIAL_PAGE_SIZE: usize = 16;
    const MAX_PAGES: usize = 6;
    const MAX_THREADS: usize = 128;
    const RESERVED_BITS: usize = 12;
}
