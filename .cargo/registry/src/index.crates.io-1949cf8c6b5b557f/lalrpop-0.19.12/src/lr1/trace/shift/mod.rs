use crate::grammar::repr::*;
use crate::lr1::core::*;

use super::trace_graph::*;
use super::Tracer;

#[cfg(test)]
mod test;

/// A backtrace explaining how a particular shift:
///
///    X = ...p (*) Token ...
///
/// came to be in the list of items for some state S. This backtrace
/// always has a particular form. First, we can walk back over the
/// prefix, which will bring us to some set of states S1 all of which
/// contain the same item, but with the cursor at the front:
///
///    X = (*) ...p Token ...
///
/// Then we can walk back within those states some number of epsilon
/// moves, traversing nonterminals of the form:
///
///    Y = (*) X ...s
///
/// (Note that each nonterminal `Y` may potentially have many
/// productions of this form. I am not sure yet if they all matter or
/// not.)
///
/// Finally, either we are in the start state, or else we reach some
/// production of the form:
///
///    Z = ...p (*) Y ...s
///
/// Ultimately this "trace" is best represented as a DAG. The problem
/// is that some of those nonterminals could, for example, be
/// optional.

impl<'trace, 'grammar> Tracer<'trace, 'grammar> {
    pub fn backtrace_shift(
        mut self,
        item_state: StateIndex,
        item: LR0Item<'grammar>,
    ) -> TraceGraph<'grammar> {
        let symbol_sets = item.symbol_sets();

        // The states `S`
        let pred_states = self.state_graph.trace_back(item_state, symbol_sets.prefix);

        // Add the edge `[X] -{...p,Token,...s}-> [X = ...p (*) Token ...s]`
        self.trace_graph
            .add_edge(item.production.nonterminal.clone(), item, symbol_sets);

        for pred_state in pred_states {
            self.trace_epsilon_edges(pred_state, &item.production.nonterminal);
        }

        self.trace_graph
    }

    // Because item.index is 0, we know we are at an index
    // like:
    //
    //     Y = (*) ...
    //
    // This can only arise if `Y` is the start nonterminal
    // or if there is an epsilon move from another item
    // like:
    //
    //     Z = ...p (*) Y ...
    //
    // So search for items like Z.
    fn trace_epsilon_edges(&mut self, item_state: StateIndex, nonterminal: &NonterminalString) // "Y"
    {
        if self.visited_set.insert((item_state, nonterminal.clone())) {
            for pred_item in self.states[item_state.0].items.vec.iter() {
                if pred_item.can_shift_nonterminal(nonterminal) {
                    if pred_item.index > 0 {
                        // Add an edge:
                        //
                        //     [Z = ...p (*) Y ...s] -(...p,Y,...s)-> [Y]
                        self.trace_graph.add_edge(
                            pred_item,
                            nonterminal.clone(),
                            pred_item.symbol_sets(),
                        );
                    } else {
                        // Trace back any incoming edges to [Z = ...p (*) Y ...].
                        let pred_nonterminal = &pred_item.production.nonterminal;
                        self.trace_graph.add_edge(
                            pred_nonterminal.clone(),
                            nonterminal.clone(),
                            pred_item.symbol_sets(),
                        );
                        self.trace_epsilon_edges(item_state, pred_nonterminal);
                    }
                }
            }
        }
    }
}
