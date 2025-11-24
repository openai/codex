use crate::collections::{set, Set};
use crate::grammar::repr::*;
use crate::lr1::core::*;
use crate::lr1::first::FirstSets;
use crate::lr1::state_graph::StateGraph;

mod reduce;
mod shift;
mod trace_graph;

pub struct Tracer<'trace, 'grammar: 'trace> {
    states: &'trace [LR1State<'grammar>],
    first_sets: &'trace FirstSets,
    state_graph: StateGraph,
    trace_graph: TraceGraph<'grammar>,
    visited_set: Set<(StateIndex, NonterminalString)>,
}

impl<'trace, 'grammar> Tracer<'trace, 'grammar> {
    pub fn new(first_sets: &'trace FirstSets, states: &'trace [LR1State<'grammar>]) -> Self {
        Tracer {
            states,
            first_sets,
            state_graph: StateGraph::new(states),
            trace_graph: TraceGraph::new(),
            visited_set: set(),
        }
    }
}

pub use self::trace_graph::TraceGraph;
