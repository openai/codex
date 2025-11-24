use crate::grammar::repr::*;
use crate::lr1::core::*;

use super::trace_graph::*;
use super::Tracer;

#[cfg(test)]
mod test;

impl<'trace, 'grammar> Tracer<'trace, 'grammar> {
    pub fn backtrace_reduce(
        mut self,
        item_state: StateIndex,
        item: LR0Item<'grammar>,
    ) -> TraceGraph<'grammar> {
        self.trace_reduce_item(item_state, item);
        self.trace_graph
    }

    fn trace_reduce_item(&mut self, item_state: StateIndex, item: LR0Item<'grammar>) {
        // We start out with an item
        //
        //     X = ...p (*) ...s
        //
        // which we can (eventually) reduce, though we may have to do
        // some epsilon reductions first if ...s is non-empty. We want
        // to trace back until we have (at least) one element of
        // context for the reduction.
        let nonterminal = &item.production.nonterminal; // X

        // Add an edge
        //
        //     [X] -{...p,_,...s}-> [X = ...p (*) ...s]
        //
        // because to reach that item we pushed `...p` from the start
        // of `X` and afterwards we expect to see `...s`.
        self.trace_graph
            .add_edge(nonterminal.clone(), item, item.symbol_sets());

        // Walk back to the set of states S where we had:
        //
        //     X = (*) ...p
        let pred_states = self.state_graph.trace_back(item_state, item.prefix());

        // Add in edges from [X] to all the places [X] can be consumed.
        for pred_state in pred_states {
            self.trace_reduce_from_state(pred_state, nonterminal);
        }
    }

    // We know that we can reduce the nonterminal `Y`. We want to find
    // at least one element of context, so we search back to find out
    // who will consume that reduced value. So search for those items
    // that can shift a `Y`:
    //
    //     Z = ... (*) Y ...s
    //
    // If we find that `...s` is potentially empty, then we haven't
    // actually found any context, and so we may have to keep
    // searching.
    fn trace_reduce_from_state(&mut self, item_state: StateIndex, nonterminal: &NonterminalString)
    // "Y"
    {
        if !self.visited_set.insert((item_state, nonterminal.clone())) {
            return;
        }
        for pred_item in self.states[item_state.0]
            .items
            .vec
            .iter()
            .filter(|i| i.can_shift_nonterminal(nonterminal))
        {
            // Found a state:
            //
            //     Z = ...p (*) Y ...s
            //
            // If `...s` does not match `\epsilon`, then we are done,
            // because `FIRST(...s)` will provide a token of context.
            // But otherwise we have to keep searching backwards.

            let symbol_sets = pred_item.symbol_sets();

            let first_suffix = self.first_sets.first0(symbol_sets.suffix);
            let continue_tracing = first_suffix.contains_eof();

            if !continue_tracing {
                // Add an edge
                //
                //    [Z = ...p (*) Y ...s] -(...p,Y,...s)-> [Y]
                //
                // and stop.
                self.trace_graph
                    .add_edge(pred_item.to_lr0(), nonterminal.clone(), symbol_sets);
            } else {
                // Add an edge
                //
                //    [Z] -{..p}-> [Y]
                //
                // because we can reduce by consuming `...p`
                // tokens, and continue tracing.
                self.trace_graph.add_edge(
                    pred_item.production.nonterminal.clone(),
                    nonterminal.clone(),
                    symbol_sets,
                );

                self.trace_reduce_item(item_state, pred_item.to_lr0());
            }
        }
    }
}
