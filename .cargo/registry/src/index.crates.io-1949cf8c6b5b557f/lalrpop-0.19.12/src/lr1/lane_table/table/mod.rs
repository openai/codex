//! The "Lane Table". In the paper, this is depicted like so:
//!
//! ```
//! +-------+----+-----+----+------------+
//! + State | C1 | ... | Cn | Successors |
//! +-------+----+-----+----+------------+
//! ```
//!
//! where each row summarizes some state that potentially contributes
//! lookahead to the conflict. The columns `Ci` represent each of the
//! conflicts we are trying to disentangle; their values are each
//! `TokenSet` indicating the lookahead contributing by this state.
//! The Successors is a vector of further successors. For simplicity
//! though we store this using maps, at least for now.

use crate::collections::{Map, Multimap, Set};
use crate::grammar::repr::*;
use crate::lr1::core::*;
use crate::lr1::lookahead::*;
use std::default::Default;
use std::fmt::{Debug, Error, Formatter};
use std::iter;

pub mod context_set;
use self::context_set::{ContextSet, OverlappingLookahead};

#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct ConflictIndex {
    index: usize,
}

impl ConflictIndex {
    pub fn new(index: usize) -> ConflictIndex {
        ConflictIndex { index }
    }
}

pub struct LaneTable<'grammar> {
    _grammar: &'grammar Grammar,
    conflicts: usize,
    lookaheads: Map<(StateIndex, ConflictIndex), TokenSet>,
    successors: Multimap<StateIndex, Set<StateIndex>>,
}

impl<'grammar> LaneTable<'grammar> {
    pub fn new(grammar: &'grammar Grammar, conflicts: usize) -> LaneTable {
        LaneTable {
            _grammar: grammar,
            conflicts,
            lookaheads: Map::default(),
            successors: Multimap::default(),
        }
    }

    pub fn add_lookahead(&mut self, state: StateIndex, conflict: ConflictIndex, tokens: &TokenSet) {
        self.lookaheads
            .entry((state, conflict))
            .or_insert_with(TokenSet::new)
            .union_with(tokens);
    }

    pub fn add_successor(&mut self, state: StateIndex, succ: StateIndex) {
        self.successors.push(state, succ);
    }

    /// Unions together the lookaheads for each column and returns a
    /// context set containing all of them. For an LALR(1) grammar,
    /// these token sets will be mutually disjoint, as discussed in
    /// the [README]; otherwise `Err` will be returned.
    ///
    /// [README]: ../README.md
    pub fn columns(&self) -> Result<ContextSet, OverlappingLookahead> {
        let mut columns = ContextSet::new(self.conflicts);
        for (&(_, conflict_index), set) in &self.lookaheads {
            columns.insert(conflict_index, set)?;
        }
        Ok(columns)
    }

    pub fn successors(&self, state: StateIndex) -> Option<&Set<StateIndex>> {
        self.successors.get(&state)
    }

    /// Returns the state of states in the table that are **not**
    /// reachable from another state in the table. These are called
    /// "beachhead states".
    pub fn beachhead_states(&self) -> Set<StateIndex> {
        // set of all states that are reachable from another state
        let reachable: Set<StateIndex> = self
            .successors
            .iter()
            .flat_map(|(_pred, succ)| succ)
            .cloned()
            .collect();

        self.lookaheads
            .keys()
            .map(|&(state_index, _)| state_index)
            .filter(|s| !reachable.contains(s))
            .collect()
    }

    pub fn context_set(&self, state: StateIndex) -> Result<ContextSet, OverlappingLookahead> {
        let mut set = ContextSet::new(self.conflicts);
        for (&(state_index, conflict_index), token_set) in &self.lookaheads {
            if state_index == state {
                set.insert(conflict_index, token_set)?;
            }
        }
        Ok(set)
    }

    /// Returns a map containing all states that appear in the table,
    /// along with the context set for each state (i.e., each row in
    /// the table, basically). Returns Err if any state has a conflict
    /// between the context sets even within its own row.
    pub fn rows(&self) -> Result<Map<StateIndex, ContextSet>, StateIndex> {
        let mut map = Map::new();
        for (&(state_index, conflict_index), token_set) in &self.lookaheads {
            debug!(
                "rows: inserting state_index={:?} conflict_index={:?} token_set={:?}",
                state_index, conflict_index, token_set
            );
            match {
                map.entry(state_index)
                    .or_insert_with(|| ContextSet::new(self.conflicts))
                    .insert(conflict_index, token_set)
            } {
                Ok(_changed) => {}
                Err(OverlappingLookahead) => {
                    debug!("rows: intra-row conflict inserting state_index={:?} conflict_index={:?} token_set={:?}",
                           state_index, conflict_index, token_set);
                    return Err(state_index);
                }
            }
        }

        // In some cases, there are states that have no context at
        // all, only successors. In that case, make sure to add an
        // empty row for them.
        for (&state_index, _) in &self.successors {
            map.entry(state_index)
                .or_insert_with(|| ContextSet::new(self.conflicts));
        }

        Ok(map)
    }
}

impl<'grammar> Debug for LaneTable<'grammar> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        let indices: Set<StateIndex> = self
            .lookaheads
            .keys()
            .map(|&(state, _)| state)
            .chain(self.successors.iter().map(|(&key, _)| key))
            .collect();

        let header = iter::once("State".to_string())
            .chain((0..self.conflicts).map(|i| format!("C{}", i)))
            .chain(Some("Successors".to_string()))
            .collect();

        let rows = indices.iter().map(|&index| {
            iter::once(format!("{:?}", index))
                .chain((0..self.conflicts).map(|i| {
                    self.lookaheads
                        .get(&(index, ConflictIndex::new(i)))
                        .map(|token_set| format!("{:?}", token_set))
                        .unwrap_or_else(String::new)
                }))
                .chain(Some(
                    self.successors
                        .get(&index)
                        .map(|c| format!("{:?}", c))
                        .unwrap_or_else(String::new),
                ))
                .collect()
        });

        let table: Vec<Vec<_>> = iter::once(header).chain(rows).collect();

        let columns = 2 + self.conflicts;

        let widths: Vec<_> = (0..columns)
            .map(|c| {
                // find the max width of any row at this column
                table.iter().map(|r| r[c].len()).max().unwrap()
            })
            .collect();

        for row in &table {
            write!(fmt, "| ")?;
            for (i, column) in row.iter().enumerate() {
                if i > 0 {
                    write!(fmt, " | ")?;
                }
                write!(fmt, "{0:1$}", column, widths[i])?;
            }
            writeln!(fmt, " |")?;
        }

        Ok(())
    }
}
