//! Mega naive LALR(1) generation algorithm.

use crate::collections::{map, Map, Multimap};
use crate::grammar::repr::*;
use crate::lr1::build;
use crate::lr1::core::*;
use crate::lr1::lookahead::*;
use crate::tls::Tls;
use itertools::Itertools;


#[cfg(test)]
mod test;

// Intermediate LALR(1) state. Identical to an LR(1) state, but that
// the items can be pushed to. We initially create these with an empty
// set of actions, as well.
struct LALR1State<'grammar> {
    pub index: StateIndex,
    pub items: Vec<LR1Item<'grammar>>,
    pub shifts: Map<TerminalString, StateIndex>,
    pub reductions: Multimap<&'grammar Production, TokenSet>,
    pub gotos: Map<NonterminalString, StateIndex>,
}

pub fn build_lalr_states<'grammar>(
    grammar: &'grammar Grammar,
    start: NonterminalString,
) -> LR1Result<'grammar> {
    // First build the LR(1) states
    let lr_states = build::build_lr1_states(grammar, start)?;

    // With lane table, there is no reason to do state collapse
    // for LALR. In fact, LALR is pointless!
    if build::use_lane_table() {
        println!("Warning: Now that the new lane-table algorithm is the default,");
        println!("         #[lalr] mode has no effect and can be removed.");
        return Ok(lr_states);
    }

    let lr1_states = profile! {
        &Tls::session(),
        "LALR(1) state collapse",
        collapse_to_lalr_states(&lr_states)
    }?;

    Ok(lr1_states)
}

pub fn collapse_to_lalr_states<'grammar>(lr_states: &[LR1State<'grammar>]) -> LR1Result<'grammar> {
    // Now compress them. This vector stores, for each state, the
    // LALR(1) state to which we will remap it.
    let mut remap: Vec<_> = (0..lr_states.len()).map(|_| StateIndex(0)).collect();
    let mut lalr1_map: Map<Vec<LR0Item>, StateIndex> = map();
    let mut lalr1_states: Vec<LALR1State> = vec![];

    for (lr1_index, lr1_state) in lr_states.iter().enumerate() {
        let lr0_kernel: Vec<_> = lr1_state
            .items
            .vec
            .iter()
            .map(Item::to_lr0)
            .dedup()
            .collect();

        let lalr1_index = *lalr1_map.entry(lr0_kernel).or_insert_with(|| {
            let index = StateIndex(lalr1_states.len());
            lalr1_states.push(LALR1State {
                index,
                items: vec![],
                shifts: map(),
                reductions: Multimap::new(),
                gotos: map(),
            });
            index
        });

        lalr1_states[lalr1_index.0]
            .items
            .extend(lr1_state.items.vec.iter().cloned());

        remap[lr1_index] = lalr1_index;
    }

    // The reduction process can leave us with multiple
    // overlapping LR(0) items, whose lookaheads must be
    // unioned. e.g. we may now have:
    //
    //     X = "(" (*) ")" ["Foo"]
    //     X = "(" (*) ")" ["Bar"]
    //
    // which we will convert to:
    //
    //     X = "(" (*) ")" ["Foo", "Bar"]
    for lalr1_state in &mut lalr1_states {
        let items = std::mem::take(&mut lalr1_state.items);

        let items: Multimap<LR0Item<'grammar>, TokenSet> = items
            .into_iter()
            .map(
                |Item {
                     production,
                     index,
                     lookahead,
                 }| { (Item::lr0(production, index), lookahead) },
            )
            .collect();

        lalr1_state.items = items
            .into_iter()
            .map(|(lr0_item, lookahead)| lr0_item.with_lookahead(lookahead))
            .collect();
    }

    // Now that items are fully built, create the actions
    for (lr1_index, lr1_state) in lr_states.iter().enumerate() {
        let lalr1_index = remap[lr1_index];
        let lalr1_state = &mut lalr1_states[lalr1_index.0];

        for (terminal, &lr1_state) in &lr1_state.shifts {
            let target_state = remap[lr1_state.0];
            let prev = lalr1_state.shifts.insert(terminal.clone(), target_state);
            assert!(prev.unwrap_or(target_state) == target_state);
        }

        for (nt, lr1_state) in &lr1_state.gotos {
            let target_state = remap[lr1_state.0];
            let prev = lalr1_state.gotos.insert(nt.clone(), target_state);
            assert!(prev.unwrap_or(target_state) == target_state); // as above
        }

        for &(ref token_set, production) in &lr1_state.reductions {
            lalr1_state.reductions.push(production, token_set.clone());
        }
    }

    // Finally, create the new states and detect conflicts
    let lr1_states: Vec<_> = lalr1_states
        .into_iter()
        .map(|lr| State {
            index: lr.index,
            items: Items { vec: lr.items },
            shifts: lr.shifts,
            reductions: lr.reductions.into_iter().map(|(p, ts)| (ts, p)).collect(),
            gotos: lr.gotos,
        })
        .collect();

    let conflicts: Vec<_> = lr1_states
        .iter()
        .flat_map(TokenSet::conflicts)
        .collect();

    if !conflicts.is_empty() {
        Err(TableConstructionError {
            states: lr1_states,
            conflicts,
        })
    } else {
        Ok(lr1_states)
    }
}
