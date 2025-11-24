//! Code to trace out a single lane, collecting information into the
//! lane table as we go.

use crate::collections::Set;
use crate::grammar::repr::*;
use crate::lr1::core::*;
use crate::lr1::first::FirstSets;
use crate::lr1::lookahead::*;
use crate::lr1::state_graph::StateGraph;

use super::table::{ConflictIndex, LaneTable};

pub struct LaneTracer<'trace, 'grammar: 'trace, L: Lookahead + 'trace> {
    states: &'trace [State<'grammar, L>],
    first_sets: &'trace FirstSets,
    state_graph: &'trace StateGraph,
    table: LaneTable<'grammar>,
    start_nt: NonterminalString,
}

impl<'trace, 'grammar, L: Lookahead> LaneTracer<'trace, 'grammar, L> {
    pub fn new(
        grammar: &'grammar Grammar,
        start_nt: NonterminalString,
        states: &'trace [State<'grammar, L>],
        first_sets: &'trace FirstSets,
        state_graph: &'trace StateGraph,
        conflicts: usize,
    ) -> Self {
        LaneTracer {
            states,
            first_sets,
            state_graph,
            start_nt,
            table: LaneTable::new(grammar, conflicts),
        }
    }

    pub fn into_table(self) -> LaneTable<'grammar> {
        self.table
    }

    pub fn start_trace(
        &mut self,
        state: StateIndex,
        conflict: ConflictIndex,
        action: Action<'grammar>,
    ) {
        let mut visited_set = Set::default();

        // if the conflict item is a "shift" item, then the context
        // is always the terminal to shift (and conflicts only arise
        // around shifting terminal, so it must be a terminal)
        match action {
            Action::Shift(term, _) => {
                let mut token_set = TokenSet::new();
                token_set.insert(Token::Terminal(term));
                self.table.add_lookahead(state, conflict, &token_set);
            }

            Action::Reduce(prod) => {
                let item = Item::lr0(prod, prod.symbols.len());
                self.continue_trace(state, conflict, item, &mut visited_set);
            }
        }
    }

    fn continue_trace(
        &mut self,
        state: StateIndex,
        conflict: ConflictIndex,
        item: LR0Item<'grammar>,
        visited: &mut Set<(StateIndex, LR0Item<'grammar>)>,
    ) {
        if !visited.insert((state, item)) {
            return;
        }

        if item.index > 0 {
            // This item was reached by shifting some symbol.  We need
            // to unshift that symbol, which means we walk backwards
            // to predecessors of `state` in the state graph.
            //
            // Example:
            //
            //     X = ...p T (*) ...s
            //
            // Here we would be "unshifting" T, which means we will
            // walk to predecessors of the current state that were
            // reached by shifting T. Those predecessors will contain
            // an item like `X = ...p (*) T ...s`, which we will then
            // process in turn.
            let shifted_symbol = item.production.symbols[item.index - 1].clone();
            let unshifted_item = Item {
                index: item.index - 1,
                ..item
            };
            let predecessors = self.state_graph.predecessors(state, shifted_symbol);
            for predecessor in predecessors {
                self.table.add_successor(predecessor, state);
                self.continue_trace(predecessor, conflict, unshifted_item, visited);
            }
            return;
        }

        // Either: we are in the start state, or this item was
        // reached by an epsilon transition. We have to
        // "unepsilon", which means that we search elsewhere in
        // the state for where the epsilon transition could have
        // come from.
        //
        // Example:
        //
        //     X = (*) ...
        //
        // We will search for other items in the same state like:
        //
        //     Y = ...p (*) X ...s
        //
        // We can then insert `FIRST(...s)` as lookahead for
        // `conflict`. If `...s` may derive epsilon, though, we
        // have to recurse and search with the previous item.

        let state_items = &self.states[state.0].items.vec;
        let nonterminal = &item.production.nonterminal;
        if *nonterminal == self.start_nt {
            // as a special case, if the `X` above is the special, synthetic
            // start-terminal, then the only thing that comes afterwards is EOF.
            self.table.add_lookahead(state, conflict, &TokenSet::eof());
        }

        // NB: Under the normal LR terms, the start nonterminal will
        // only have one production like `X' = X`, in which case this
        // loop is useless, but sometimes in tests we don't observe
        // that restriction, so do it anyway.
        for pred_item in state_items
            .iter()
            .filter(|i| i.can_shift_nonterminal(nonterminal))
        {
            let symbol_sets = pred_item.symbol_sets();
            let mut first = self.first_sets.first0(symbol_sets.suffix);
            let derives_epsilon = first.take_eof();
            self.table.add_lookahead(state, conflict, &first);
            if derives_epsilon {
                self.continue_trace(state, conflict, pred_item.to_lr0(), visited);
            }
        }
    }
}
