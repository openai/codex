use std::{
    collections::{HashMap, HashSet},
    mem,
};

use log::info;

use super::token_conflicts::TokenConflictMap;
use crate::{
    dedup::split_state_id_groups,
    grammars::{LexicalGrammar, SyntaxGrammar, VariableType},
    rules::{AliasMap, Symbol, TokenSet},
    tables::{GotoAction, ParseAction, ParseState, ParseStateId, ParseTable, ParseTableEntry},
};

pub fn minimize_parse_table(
    parse_table: &mut ParseTable,
    syntax_grammar: &SyntaxGrammar,
    lexical_grammar: &LexicalGrammar,
    simple_aliases: &AliasMap,
    token_conflict_map: &TokenConflictMap,
    keywords: &TokenSet,
) {
    let mut minimizer = Minimizer {
        parse_table,
        syntax_grammar,
        lexical_grammar,
        token_conflict_map,
        keywords,
        simple_aliases,
    };
    minimizer.merge_compatible_states();
    minimizer.remove_unit_reductions();
    minimizer.remove_unused_states();
    minimizer.reorder_states_by_descending_size();
}

struct Minimizer<'a> {
    parse_table: &'a mut ParseTable,
    syntax_grammar: &'a SyntaxGrammar,
    lexical_grammar: &'a LexicalGrammar,
    token_conflict_map: &'a TokenConflictMap<'a>,
    keywords: &'a TokenSet,
    simple_aliases: &'a AliasMap,
}

impl Minimizer<'_> {
    fn remove_unit_reductions(&mut self) {
        let mut aliased_symbols = HashSet::new();
        for variable in &self.syntax_grammar.variables {
            for production in &variable.productions {
                for step in &production.steps {
                    if step.alias.is_some() {
                        aliased_symbols.insert(step.symbol);
                    }
                }
            }
        }

        let mut unit_reduction_symbols_by_state = HashMap::new();
        for (i, state) in self.parse_table.states.iter().enumerate() {
            let mut only_unit_reductions = true;
            let mut unit_reduction_symbol = None;
            for (_, entry) in &state.terminal_entries {
                for action in &entry.actions {
                    match action {
                        ParseAction::ShiftExtra => continue,
                        ParseAction::Reduce {
                            child_count: 1,
                            production_id: 0,
                            symbol,
                            ..
                        } if !self.simple_aliases.contains_key(symbol)
                            && !self.syntax_grammar.supertype_symbols.contains(symbol)
                            && !self.syntax_grammar.extra_symbols.contains(symbol)
                            && !aliased_symbols.contains(symbol)
                            && self.syntax_grammar.variables[symbol.index].kind
                                != VariableType::Named
                            && (unit_reduction_symbol.is_none()
                                || unit_reduction_symbol == Some(symbol)) =>
                        {
                            unit_reduction_symbol = Some(symbol);
                            continue;
                        }
                        _ => {}
                    }
                    only_unit_reductions = false;
                    break;
                }

                if !only_unit_reductions {
                    break;
                }
            }

            if let Some(symbol) = unit_reduction_symbol {
                if only_unit_reductions {
                    unit_reduction_symbols_by_state.insert(i, *symbol);
                }
            }
        }

        for state in &mut self.parse_table.states {
            let mut done = false;
            while !done {
                done = true;
                state.update_referenced_states(|other_state_id, state| {
                    unit_reduction_symbols_by_state.get(&other_state_id).map_or(
                        other_state_id,
                        |symbol| {
                            done = false;
                            match state.nonterminal_entries.get(symbol) {
                                Some(GotoAction::Goto(state_id)) => *state_id,
                                _ => other_state_id,
                            }
                        },
                    )
                });
            }
        }
    }

    fn merge_compatible_states(&mut self) {
        let core_count = 1 + self
            .parse_table
            .states
            .iter()
            .map(|state| state.core_id)
            .max()
            .unwrap();

        // Initially group the states by their parse item set core.
        let mut group_ids_by_state_id = Vec::with_capacity(self.parse_table.states.len());
        let mut state_ids_by_group_id = vec![Vec::<ParseStateId>::new(); core_count];
        for (i, state) in self.parse_table.states.iter().enumerate() {
            state_ids_by_group_id[state.core_id].push(i);
            group_ids_by_state_id.push(state.core_id);
        }

        split_state_id_groups(
            &self.parse_table.states,
            &mut state_ids_by_group_id,
            &mut group_ids_by_state_id,
            0,
            |left, right, groups| self.states_conflict(left, right, groups),
        );

        while split_state_id_groups(
            &self.parse_table.states,
            &mut state_ids_by_group_id,
            &mut group_ids_by_state_id,
            0,
            |left, right, groups| self.state_successors_differ(left, right, groups),
        ) {}

        let error_group_index = state_ids_by_group_id
            .iter()
            .position(|g| g.contains(&0))
            .unwrap();
        let start_group_index = state_ids_by_group_id
            .iter()
            .position(|g| g.contains(&1))
            .unwrap();
        state_ids_by_group_id.swap(error_group_index, 0);
        state_ids_by_group_id.swap(start_group_index, 1);

        // Create a list of new parse states: one state for each group of old states.
        let mut new_states = Vec::with_capacity(state_ids_by_group_id.len());
        for state_ids in &state_ids_by_group_id {
            // Initialize the new state based on the first old state in the group.
            let mut parse_state = mem::take(&mut self.parse_table.states[state_ids[0]]);

            // Extend the new state with all of the actions from the other old states
            // in the group.
            for state_id in &state_ids[1..] {
                let other_parse_state = mem::take(&mut self.parse_table.states[*state_id]);

                parse_state
                    .terminal_entries
                    .extend(other_parse_state.terminal_entries);
                parse_state
                    .nonterminal_entries
                    .extend(other_parse_state.nonterminal_entries);
                parse_state
                    .reserved_words
                    .insert_all(&other_parse_state.reserved_words);
                for symbol in parse_state.terminal_entries.keys() {
                    parse_state.reserved_words.remove(symbol);
                }
            }

            // Update the new state's outgoing references using the new grouping.
            parse_state.update_referenced_states(|state_id, _| group_ids_by_state_id[state_id]);
            new_states.push(parse_state);
        }

        self.parse_table.states = new_states;
    }

    fn states_conflict(
        &self,
        left_state: &ParseState,
        right_state: &ParseState,
        group_ids_by_state_id: &[ParseStateId],
    ) -> bool {
        for (token, left_entry) in &left_state.terminal_entries {
            if let Some(right_entry) = right_state.terminal_entries.get(token) {
                if self.entries_conflict(
                    left_state.id,
                    right_state.id,
                    token,
                    left_entry,
                    right_entry,
                    group_ids_by_state_id,
                ) {
                    return true;
                }
            } else if self.token_conflicts(left_state.id, right_state.id, right_state, *token) {
                return true;
            }
        }

        for token in right_state.terminal_entries.keys() {
            if !left_state.terminal_entries.contains_key(token)
                && self.token_conflicts(left_state.id, right_state.id, left_state, *token)
            {
                return true;
            }
        }

        false
    }

    fn state_successors_differ(
        &self,
        state1: &ParseState,
        state2: &ParseState,
        group_ids_by_state_id: &[ParseStateId],
    ) -> bool {
        for (token, entry1) in &state1.terminal_entries {
            if let ParseAction::Shift { state: s1, .. } = entry1.actions.last().unwrap() {
                if let Some(entry2) = state2.terminal_entries.get(token) {
                    if let ParseAction::Shift { state: s2, .. } = entry2.actions.last().unwrap() {
                        let group1 = group_ids_by_state_id[*s1];
                        let group2 = group_ids_by_state_id[*s2];
                        if group1 != group2 {
                            info!(
                                "split states {} {} - successors for {} are split: {s1} {s2}",
                                state1.id,
                                state2.id,
                                self.symbol_name(token),
                            );
                            return true;
                        }
                    }
                }
            }
        }

        for (symbol, s1) in &state1.nonterminal_entries {
            if let Some(s2) = state2.nonterminal_entries.get(symbol) {
                match (s1, s2) {
                    (GotoAction::ShiftExtra, GotoAction::ShiftExtra) => {}
                    (GotoAction::Goto(s1), GotoAction::Goto(s2)) => {
                        let group1 = group_ids_by_state_id[*s1];
                        let group2 = group_ids_by_state_id[*s2];
                        if group1 != group2 {
                            info!(
                                "split states {} {} - successors for {} are split: {s1} {s2}",
                                state1.id,
                                state2.id,
                                self.symbol_name(symbol),
                            );
                            return true;
                        }
                    }
                    _ => return true,
                }
            }
        }

        false
    }

    fn entries_conflict(
        &self,
        state_id1: ParseStateId,
        state_id2: ParseStateId,
        token: &Symbol,
        entry1: &ParseTableEntry,
        entry2: &ParseTableEntry,
        group_ids_by_state_id: &[ParseStateId],
    ) -> bool {
        // To be compatible, entries need to have the same actions.
        let actions1 = &entry1.actions;
        let actions2 = &entry2.actions;
        if actions1.len() != actions2.len() {
            info!(
                "split states {state_id1} {state_id2} - differing action counts for token {}",
                self.symbol_name(token)
            );
            return true;
        }

        for (i, action1) in actions1.iter().enumerate() {
            let action2 = &actions2[i];

            // Two shift actions are equivalent if their destinations are in the same group.
            if let (
                ParseAction::Shift {
                    state: s1,
                    is_repetition: is_repetition1,
                },
                ParseAction::Shift {
                    state: s2,
                    is_repetition: is_repetition2,
                },
            ) = (action1, action2)
            {
                let group1 = group_ids_by_state_id[*s1];
                let group2 = group_ids_by_state_id[*s2];
                if group1 == group2 && is_repetition1 == is_repetition2 {
                    continue;
                }
                info!(
                    "split states {state_id1} {state_id2} - successors for {} are split: {s1} {s2}",
                    self.symbol_name(token),
                );
                return true;
            } else if action1 != action2 {
                info!(
                    "split states {state_id1} {state_id2} - unequal actions for {}",
                    self.symbol_name(token),
                );
                return true;
            }
        }

        false
    }

    fn token_conflicts(
        &self,
        left_id: ParseStateId,
        right_id: ParseStateId,
        right_state: &ParseState,
        new_token: Symbol,
    ) -> bool {
        if new_token == Symbol::end_of_nonterminal_extra() {
            info!("split states {left_id} {right_id} - end of non-terminal extra",);
            return true;
        }

        // Do not add external tokens; they could conflict lexically with any of the state's
        // existing lookahead tokens.
        if new_token.is_external() {
            info!(
                "split states {left_id} {right_id} - external token {}",
                self.symbol_name(&new_token),
            );
            return true;
        }

        if right_state.reserved_words.contains(&new_token) {
            return false;
        }

        // Do not add tokens which are both internal and external. Their validity could
        // influence the behavior of the external scanner.
        if self
            .syntax_grammar
            .external_tokens
            .iter()
            .any(|external| external.corresponding_internal_token == Some(new_token))
        {
            info!(
                "split states {left_id} {right_id} - internal/external token {}",
                self.symbol_name(&new_token),
            );
            return true;
        }

        // Do not add a token if it conflicts with an existing token.
        for token in right_state.terminal_entries.keys().copied() {
            if !token.is_terminal() {
                continue;
            }
            if self.syntax_grammar.word_token == Some(token) && self.keywords.contains(&new_token) {
                continue;
            }
            if self.syntax_grammar.word_token == Some(new_token) && self.keywords.contains(&token) {
                continue;
            }

            if self
                .token_conflict_map
                .does_conflict(new_token.index, token.index)
                || self
                    .token_conflict_map
                    .does_match_same_string(new_token.index, token.index)
            {
                info!(
                    "split states {} {} - token {} conflicts with {}",
                    left_id,
                    right_id,
                    self.symbol_name(&new_token),
                    self.symbol_name(&token),
                );
                return true;
            }
        }

        false
    }

    fn symbol_name(&self, symbol: &Symbol) -> &String {
        if symbol.is_non_terminal() {
            &self.syntax_grammar.variables[symbol.index].name
        } else if symbol.is_external() {
            &self.syntax_grammar.external_tokens[symbol.index].name
        } else {
            &self.lexical_grammar.variables[symbol.index].name
        }
    }

    fn remove_unused_states(&mut self) {
        let mut state_usage_map = vec![false; self.parse_table.states.len()];

        state_usage_map[0] = true;
        state_usage_map[1] = true;

        for state in &self.parse_table.states {
            for referenced_state in state.referenced_states() {
                state_usage_map[referenced_state] = true;
            }
        }
        let mut removed_predecessor_count = 0;
        let mut state_replacement_map = vec![0; self.parse_table.states.len()];
        for state_id in 0..self.parse_table.states.len() {
            state_replacement_map[state_id] = state_id - removed_predecessor_count;
            if !state_usage_map[state_id] {
                removed_predecessor_count += 1;
            }
        }
        let mut state_id = 0;
        let mut original_state_id = 0;
        while state_id < self.parse_table.states.len() {
            if state_usage_map[original_state_id] {
                self.parse_table.states[state_id].update_referenced_states(|other_state_id, _| {
                    state_replacement_map[other_state_id]
                });
                state_id += 1;
            } else {
                self.parse_table.states.remove(state_id);
            }
            original_state_id += 1;
        }
    }

    fn reorder_states_by_descending_size(&mut self) {
        // Get a mapping of old state index -> new_state_index
        let mut old_ids_by_new_id = (0..self.parse_table.states.len()).collect::<Vec<_>>();
        old_ids_by_new_id.sort_unstable_by_key(|i| {
            // Don't changes states 0 (the error state) or 1 (the start state).
            if *i <= 1 {
                return *i as i64 - 1_000_000;
            }

            // Reorder all the other states by descending symbol count.
            let state = &self.parse_table.states[*i];
            -((state.terminal_entries.len() + state.nonterminal_entries.len()) as i64)
        });

        // Get the inverse mapping
        let mut new_ids_by_old_id = vec![0; old_ids_by_new_id.len()];
        for (id, old_id) in old_ids_by_new_id.iter().enumerate() {
            new_ids_by_old_id[*old_id] = id;
        }

        // Reorder the parse states and update their references to reflect
        // the new ordering.
        self.parse_table.states = old_ids_by_new_id
            .iter()
            .map(|old_id| {
                let mut state = ParseState::default();
                mem::swap(&mut state, &mut self.parse_table.states[*old_id]);
                state.update_referenced_states(|id, _| new_ids_by_old_id[id]);
                state
            })
            .collect();
    }
}
