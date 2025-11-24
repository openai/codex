//!

use crate::collections::{Map, Set};
use crate::grammar::repr::*;
use crate::lr1::build;
use crate::lr1::core::*;
use crate::lr1::first::FirstSets;
use crate::lr1::lane_table::lane::LaneTracer;
use crate::lr1::lane_table::table::context_set::OverlappingLookahead;
use crate::lr1::lane_table::table::{ConflictIndex, LaneTable};
use crate::lr1::lookahead::{Lookahead, TokenSet};
use crate::lr1::state_graph::StateGraph;
use ena::unify::InPlaceUnificationTable;

mod merge;
use self::merge::Merge;

mod state_set;
use self::state_set::StateSet;

pub struct LaneTableConstruct<'grammar> {
    grammar: &'grammar Grammar,
    first_sets: FirstSets,
    start_nt: NonterminalString,
}

impl<'grammar> LaneTableConstruct<'grammar> {
    pub fn new(grammar: &'grammar Grammar, start_nt: NonterminalString) -> Self {
        let first_sets = FirstSets::new(grammar);
        LaneTableConstruct {
            grammar,
            start_nt,
            first_sets,
        }
    }

    pub fn construct(self) -> Result<Vec<LR1State<'grammar>>, LR1TableConstructionError<'grammar>> {
        let states = {
            match build::build_lr0_states(self.grammar, self.start_nt.clone()) {
                Ok(states) => {
                    // In this case, the grammar is actually
                    // LR(0). This is very rare -- it means that the
                    // grammar does not need lookahead to execute. In
                    // principle, we could stop here, except that if
                    // we do so, then the lookahead values that we get
                    // are very broad.
                    //
                    // Broad lookahead values will cause "eager"
                    // reduce at runtime -- i.e., if there is some
                    // scenario where the lookahead tells you we are
                    // in error, but we would have to reduce a few
                    // states before we see it. This, in turn, can
                    // cause infinite loops around error recovery
                    // (#240).
                    //
                    // Since we want to behave as a LR(1) parser
                    // would, we'll just go ahead and run the
                    // algorithm.
                    states
                }
                Err(TableConstructionError { states, .. }) => states,
            }
        };

        // Convert the LR(0) states into LR(0-1) states.
        let mut states = self.promote_lr0_states(states);

        // For each inconsistent state, apply the lane-table algorithm to
        // resolve it.
        for i in 0.. {
            if i >= states.len() {
                break;
            }

            match self.resolve_inconsistencies(&mut states, StateIndex(i)) {
                Ok(()) => {}
                Err(_) => {
                    // We failed because of irreconcilable conflicts
                    // somewhere. Just compute the conflicts from the final set of
                    // states.
                    debug!(
                        "construct: failed to resolve inconsistencies in state {:#?}",
                        states[i]
                    );
                    let conflicts: Vec<Conflict<'grammar, TokenSet>> = states
                        .iter()
                        .flat_map(Lookahead::conflicts)
                        .collect();
                    return Err(TableConstructionError { states, conflicts });
                }
            }
        }

        Ok(states)
    }

    /// Given a set of LR0 states, returns LR1 states where the lookahead
    /// is always `TokenSet::all()`. We refer to these states as LR(0-1)
    /// states in the README.
    fn promote_lr0_states(&self, lr0: Vec<LR0State<'grammar>>) -> Vec<LR1State<'grammar>> {
        let all = TokenSet::all();
        debug!("promote_lr0_states: all={:?}", all);
        lr0.into_iter()
            .map(|s| {
                let items = s
                    .items
                    .vec
                    .iter()
                    .map(|item| Item {
                        production: item.production,
                        index: item.index,
                        lookahead: all.clone(),
                    })
                    .collect();
                let reductions = s
                    .reductions
                    .into_iter()
                    .map(|(_, p)| (all.clone(), p))
                    .collect();
                State {
                    index: s.index,
                    items: Items { vec: items },
                    shifts: s.shifts,
                    reductions,
                    gotos: s.gotos,
                }
            })
            .collect()
    }

    fn resolve_inconsistencies(
        &self,
        states: &mut Vec<LR1State<'grammar>>,
        inconsistent_state: StateIndex,
    ) -> Result<(), StateIndex> {
        debug!(
            "resolve_inconsistencies(inconsistent_state={:?}/{:#?}",
            inconsistent_state, states[inconsistent_state.0]
        );

        let mut actions = super::conflicting_actions(&states[inconsistent_state.0]);
        if actions.is_empty() {
            // This can mean one of two things: only shifts, or a
            // single reduction. We have to be careful about states
            // with a single reduction: even though such a state is
            // not inconsistent (there is only one possible course of
            // action), we still want to run the lane table algorithm,
            // because otherwise we get states with "complete"
            // lookahead, which messes with error recovery.
            //
            // In particular, if there is too much lookahead, we will
            // reduce even when it is inappropriate to do so.
            actions = states[inconsistent_state.0]
                .reductions
                .iter()
                .map(|&(_, prod)| Action::Reduce(prod))
                .collect();
            if actions.is_empty() {
                return Ok(());
            }
        }

        debug!("resolve_inconsistencies: conflicting_actions={:?}", actions);

        let table = self.build_lane_table(states, inconsistent_state, &actions);

        // Consider first the "LALR" case, where the lookaheads for each
        // action are completely disjoint.
        if self.attempt_lalr(&mut states[inconsistent_state.0], &table, &actions) {
            return Ok(());
        }

        // Construct the initial states; each state will map to a
        // context-set derived from its row in the lane-table. This is
        // fallible, because a state may be internally inconsistent.
        //
        // (To handle unification, we also map each state to a
        // `StateSet` that is its entry in the `ena` table.)
        let rows = table.rows()?;
        let mut unify = InPlaceUnificationTable::<StateSet>::new();
        let mut state_sets = Map::new();
        for (&state_index, context_set) in &rows {
            let state_set = unify.new_key(context_set.clone());
            state_sets.insert(state_index, state_set);
            debug!(
                "resolve_inconsistencies: state_index={:?}, state_set={:?}",
                state_index, state_set
            );
        }

        // Now merge state-sets, cloning states where needed.
        let mut merge = Merge::new(
            &table,
            &mut unify,
            states,
            &mut state_sets,
            inconsistent_state,
        );
        let beachhead_states = table.beachhead_states();
        for beachhead_state in beachhead_states {
            match merge.start(beachhead_state) {
                Ok(()) => {}
                Err((source, _)) => {
                    debug!(
                        "resolve_inconsistencies: failed to merge, source={:?}",
                        source
                    );
                    return Err(source);
                }
            }
        }
        merge.patch_target_starts(&actions);

        Ok(())
    }

    fn attempt_lalr(
        &self,
        state: &mut LR1State<'grammar>,
        table: &LaneTable<'grammar>,
        actions: &Set<Action<'grammar>>,
    ) -> bool {
        match table.columns() {
            Ok(columns) => {
                debug!("attempt_lalr, columns={:#?}", columns);
                columns.apply(state, actions);
                debug!("attempt_lalr, state={:#?}", state);
                true
            }
            Err(OverlappingLookahead) => {
                debug!("attempt_lalr, OverlappingLookahead");
                false
            }
        }
    }

    fn build_lane_table(
        &self,
        states: &[LR1State<'grammar>],
        inconsistent_state: StateIndex,
        actions: &Set<Action<'grammar>>,
    ) -> LaneTable<'grammar> {
        let state_graph = StateGraph::new(states);
        let mut tracer = LaneTracer::new(
            self.grammar,
            self.start_nt.clone(),
            states,
            &self.first_sets,
            &state_graph,
            actions.len(),
        );
        for (i, action) in actions.iter().enumerate() {
            tracer.start_trace(inconsistent_state, ConflictIndex::new(i), action.clone());
        }
        tracer.into_table()
    }
}
