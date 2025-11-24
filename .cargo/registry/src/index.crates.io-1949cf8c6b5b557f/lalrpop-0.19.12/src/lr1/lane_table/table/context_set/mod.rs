//! A key part of the lane-table algorithm is the idea of a CONTEXT
//! SET (my name, the paper has no name for this). Basically it
//! represents the LR1 context under which a given conflicting action
//! would take place.
//!
//! So, for example, imagine this grammar:
//!
//! ```notrust
//! A = B x
//!   | C y
//! B = z
//! C = z
//! ```
//!
//! This gives rise to states like:
//!
//! - `S0 = { * B x, * C y, B = * z, C = * z }`
//! - `S1 = { B = z *, C = z * }`
//!
//! This second state has two conflicting items. Let's call them
//! conflicts 0 and 1 respectively. The conflict set would then have
//! two entries (one for each conflict) and it would map each of them
//! to a TokenSet supplying context. So when we trace everything
//! out we might get a ContextSet of:
//!
//! - `[ 0: x, 1: y ]`
//!
//! In general, you want to ensure that the token sets of all
//! conflicting items are pairwise-disjoint, or else if you get to a
//! state that has both of those items (which, by definition, does
//! arise) you won't know which to take. In this case, we're all set,
//! because item 0 occurs only with lookahead `x` and item 1 with
//! lookahead `y`.

use crate::collections::{Map, Set};
use crate::lr1::core::*;
use crate::lr1::lookahead::*;
mod test;

use super::ConflictIndex;

#[derive(Clone, Debug)]
pub struct ContextSet {
    values: Vec<TokenSet>,
}

#[derive(Debug)]
pub struct OverlappingLookahead;

impl ContextSet {
    pub fn new(num_conflicts: usize) -> Self {
        ContextSet {
            values: (0..num_conflicts).map(|_| TokenSet::new()).collect(),
        }
    }

    pub fn union(set1: &ContextSet, set2: &ContextSet) -> Result<Self, OverlappingLookahead> {
        let mut result = set1.clone();
        for (i, t) in set2.values.iter().enumerate() {
            result.insert(ConflictIndex::new(i), t)?;
        }
        Ok(result)
    }

    /// Attempts to merge the values `conflict: set` into this
    /// conflict set. If this would result in an invalid conflict set
    /// (where two conflicts have overlapping lookahead), then returns
    /// `Err(OverlappingLookahead)` and has no effect.
    ///
    /// Assuming no errors, returns `Ok(true)` if this resulted in any
    /// modifications, and `Ok(false)` otherwise.
    pub fn insert(
        &mut self,
        conflict: ConflictIndex,
        set: &TokenSet,
    ) -> Result<bool, OverlappingLookahead> {
        for (value, index) in self.values.iter().zip((0..).map(ConflictIndex::new)) {
            if index != conflict && value.is_intersecting(set) {
                return Err(OverlappingLookahead);
            }
        }

        Ok(self.values[conflict.index].union_with(set))
    }

    pub fn apply<'grammar>(&self, state: &mut LR1State<'grammar>, actions: &Set<Action<'grammar>>) {
        // create a map from each action to its lookahead
        let lookaheads: Map<Action<'grammar>, &TokenSet> =
            actions.iter().cloned().zip(&self.values).collect();

        for &mut (ref mut lookahead, production) in &mut state.reductions {
            let action = Action::Reduce(production);
            *lookahead = lookaheads[&action].clone();
        }
    }
}
