//! Naive LR(1) generation algorithm.

use crate::grammar::repr::*;

mod build;
mod build_lalr;
pub mod codegen;
mod core;
mod error;
mod example;
mod first;
mod lane_table;
mod lookahead;
mod report;
mod state_graph;
mod tls;
mod trace;
use std::io::{self, Write};

#[cfg(test)]
mod interpret;

pub use self::core::{LR1Result, LR1TableConstructionError};
pub use self::error::report_error;
pub use self::tls::Lr1Tls;

pub fn build_states<'grammar>(
    grammar: &'grammar Grammar,
    start: NonterminalString,
) -> LR1Result<'grammar> {
    let mut lr1_states = if !grammar.algorithm.lalr {
        build::build_lr1_states(grammar, start)?
    } else {
        build_lalr::build_lalr_states(grammar, start)?
    };

    rewrite_state_indices(grammar, &mut lr1_states);

    Ok(lr1_states)
}

pub fn generate_report<'grammar, W: Write + 'grammar>(
    out: &'grammar mut W,
    lr1result: &LR1Result<'grammar>,
) -> io::Result<()> {
    report::generate_report(out, lr1result)
}

/// By packing all states which start a reduction we can generate a smaller goto table as any
/// states not starting a reduction will not need a row
fn rewrite_state_indices(grammar: &Grammar, states: &mut [core::LR1State]) {
    let mut start_states = vec![false; states.len()];
    for (index, state) in states.iter_mut().enumerate() {
        debug_assert!(state.index.0 == index);
        if grammar
            .nonterminals
            .keys()
            .any(|nonterminal| state.gotos.get(nonterminal).is_some())
        {
            start_states[index] = true;
        }
    }

    // Since the sort is stable and we put starting states first, the initial state is still 0
    states.sort_by_key(|state| !start_states[state.index.0]);

    let mut state_rewrite = vec![0; states.len()];
    for (new_index, state) in states.iter_mut().enumerate() {
        state_rewrite[state.index.0] = new_index;
        state.index.0 = new_index;
    }

    for state in states {
        for goto in state.gotos.values_mut() {
            goto.0 = state_rewrite[goto.0];
        }
        for shift in state.shifts.values_mut() {
            shift.0 = state_rewrite[shift.0];
        }
    }
}
