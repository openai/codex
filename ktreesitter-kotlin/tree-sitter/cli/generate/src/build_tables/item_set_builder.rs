use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use super::item::{ParseItem, ParseItemDisplay, ParseItemSet, ParseItemSetEntry, TokenSetDisplay};
use crate::{
    grammars::{InlinedProductionMap, LexicalGrammar, ReservedWordSetId, SyntaxGrammar},
    rules::{Symbol, SymbolType, TokenSet},
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct TransitiveClosureAddition<'a> {
    item: ParseItem<'a>,
    info: FollowSetInfo,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct FollowSetInfo {
    lookaheads: TokenSet,
    reserved_lookaheads: ReservedWordSetId,
    propagates_lookaheads: bool,
}

pub struct ParseItemSetBuilder<'a> {
    syntax_grammar: &'a SyntaxGrammar,
    lexical_grammar: &'a LexicalGrammar,
    first_sets: HashMap<Symbol, TokenSet>,
    reserved_first_sets: HashMap<Symbol, ReservedWordSetId>,
    last_sets: HashMap<Symbol, TokenSet>,
    inlines: &'a InlinedProductionMap,
    transitive_closure_additions: Vec<Vec<TransitiveClosureAddition<'a>>>,
}

fn find_or_push<T: Eq>(vector: &mut Vec<T>, value: T) {
    if !vector.contains(&value) {
        vector.push(value);
    }
}

impl<'a> ParseItemSetBuilder<'a> {
    pub fn new(
        syntax_grammar: &'a SyntaxGrammar,
        lexical_grammar: &'a LexicalGrammar,
        inlines: &'a InlinedProductionMap,
    ) -> Self {
        let mut result = Self {
            syntax_grammar,
            lexical_grammar,
            first_sets: HashMap::new(),
            reserved_first_sets: HashMap::new(),
            last_sets: HashMap::new(),
            inlines,
            transitive_closure_additions: vec![Vec::new(); syntax_grammar.variables.len()],
        };

        // For each grammar symbol, populate the FIRST and LAST sets: the set of
        // terminals that appear at the beginning and end that symbol's productions,
        // respectively.
        // For a terminal symbol, the FIRST and LAST sets just consist of the
        // terminal itself.
        for i in 0..lexical_grammar.variables.len() {
            let symbol = Symbol::terminal(i);
            let mut set = TokenSet::new();
            set.insert(symbol);
            result.first_sets.insert(symbol, set.clone());
            result.last_sets.insert(symbol, set);
            result
                .reserved_first_sets
                .insert(symbol, ReservedWordSetId::default());
        }

        for i in 0..syntax_grammar.external_tokens.len() {
            let symbol = Symbol::external(i);
            let mut set = TokenSet::new();
            set.insert(symbol);
            result.first_sets.insert(symbol, set.clone());
            result.last_sets.insert(symbol, set);
            result
                .reserved_first_sets
                .insert(symbol, ReservedWordSetId::default());
        }

        // The FIRST set of a non-terminal `i` is the union of the the FIRST sets
        // of all the symbols that appear at the beginnings of i's productions. Some
        // of these symbols may themselves be non-terminals, so this is a recursive
        // definition.
        //
        // Rather than computing these sets using recursion, we use an explicit stack
        // called `symbols_to_process`.
        let mut symbols_to_process = Vec::new();
        let mut processed_non_terminals = HashSet::new();
        for i in 0..syntax_grammar.variables.len() {
            let symbol = Symbol::non_terminal(i);
            let first_set = result.first_sets.entry(symbol).or_default();
            let reserved_first_set = result.reserved_first_sets.entry(symbol).or_default();

            processed_non_terminals.clear();
            symbols_to_process.clear();
            symbols_to_process.push(symbol);
            while let Some(sym) = symbols_to_process.pop() {
                for production in &syntax_grammar.variables[sym.index].productions {
                    if let Some(step) = production.steps.first() {
                        if step.symbol.is_terminal() || step.symbol.is_external() {
                            first_set.insert(step.symbol);
                        } else if processed_non_terminals.insert(step.symbol) {
                            symbols_to_process.push(step.symbol);
                        }
                        *reserved_first_set = (*reserved_first_set).max(step.reserved_word_set_id);
                    }
                }
            }

            // The LAST set is defined in a similar way to the FIRST set.
            let last_set = result.last_sets.entry(symbol).or_default();
            processed_non_terminals.clear();
            symbols_to_process.clear();
            symbols_to_process.push(symbol);
            while let Some(sym) = symbols_to_process.pop() {
                for production in &syntax_grammar.variables[sym.index].productions {
                    if let Some(step) = production.steps.last() {
                        if step.symbol.is_terminal() || step.symbol.is_external() {
                            last_set.insert(step.symbol);
                        } else if processed_non_terminals.insert(step.symbol) {
                            symbols_to_process.push(step.symbol);
                        }
                    }
                }
            }
        }

        // To compute an item set's transitive closure, we find each item in the set
        // whose next symbol is a non-terminal, and we add new items to the set for
        // each of that symbol's productions. These productions might themselves begin
        // with non-terminals, so the process continues recursively. In this process,
        // the total set of entries that get added depends only on two things:
        //
        //   * the non-terminal symbol that occurs next in each item
        //
        //   * the set of terminals that can follow that non-terminal symbol in the item
        //
        // So we can avoid a lot of duplicated recursive work by precomputing, for each
        // non-terminal symbol `i`, a final list of *additions* that must be made to an
        // item set when symbol `i` occurs as the next symbol in one if its core items.
        // The structure of a precomputed *addition* is as follows:
        //
        //   * `item` - the new item that must be added as part of the expansion of the symbol `i`.
        //
        //   * `lookaheads` - the set of possible lookahead tokens that can always come after `item`
        //     in an expansion of symbol `i`.
        //
        //   * `reserved_lookaheads` - the set of reserved lookahead lookahead tokens that can
        //     always come after `item` in the expansion of symbol `i`.
        //
        //   * `propagates_lookaheads` - a boolean indicating whether or not `item` can occur at the
        //     *end* of the expansion of symbol `i`, so that i's own current lookahead tokens can
        //     occur after `item`.
        //
        // Rather than computing these additions recursively, we use an explicit stack.
        let empty_lookaheads = TokenSet::new();
        let mut stack = Vec::new();
        let mut follow_set_info_by_non_terminal = HashMap::<usize, FollowSetInfo>::new();
        for i in 0..syntax_grammar.variables.len() {
            // First, build up a map whose keys are all of the non-terminals that can
            // appear at the beginning of non-terminal `i`, and whose values store
            // information about the tokens that can follow those non-terminals.
            stack.clear();
            stack.push((i, &empty_lookaheads, ReservedWordSetId::default(), true));
            follow_set_info_by_non_terminal.clear();
            while let Some((sym_ix, lookaheads, reserved_word_set_id, propagates_lookaheads)) =
                stack.pop()
            {
                let mut did_add = false;
                let info = follow_set_info_by_non_terminal.entry(sym_ix).or_default();
                did_add |= info.lookaheads.insert_all(lookaheads);
                if reserved_word_set_id > info.reserved_lookaheads {
                    info.reserved_lookaheads = reserved_word_set_id;
                    did_add = true;
                }
                did_add |= propagates_lookaheads && !info.propagates_lookaheads;
                info.propagates_lookaheads |= propagates_lookaheads;
                if !did_add {
                    continue;
                }

                for production in &syntax_grammar.variables[sym_ix].productions {
                    if let Some(symbol) = production.first_symbol() {
                        if symbol.is_non_terminal() {
                            if let Some(next_step) = production.steps.get(1) {
                                stack.push((
                                    symbol.index,
                                    &result.first_sets[&next_step.symbol],
                                    result.reserved_first_sets[&next_step.symbol],
                                    false,
                                ));
                            } else {
                                stack.push((
                                    symbol.index,
                                    lookaheads,
                                    reserved_word_set_id,
                                    propagates_lookaheads,
                                ));
                            }
                        }
                    }
                }
            }

            // Store all of those non-terminals' productions, along with their associated
            // lookahead info, as *additions* associated with non-terminal `i`.
            let additions_for_non_terminal = &mut result.transitive_closure_additions[i];
            for (&variable_index, follow_set_info) in &follow_set_info_by_non_terminal {
                let variable = &syntax_grammar.variables[variable_index];
                let non_terminal = Symbol::non_terminal(variable_index);
                let variable_index = variable_index as u32;
                if syntax_grammar.variables_to_inline.contains(&non_terminal) {
                    continue;
                }
                for production in &variable.productions {
                    let item = ParseItem {
                        variable_index,
                        production,
                        step_index: 0,
                        has_preceding_inherited_fields: false,
                    };

                    if let Some(inlined_productions) =
                        inlines.inlined_productions(item.production, item.step_index)
                    {
                        for production in inlined_productions {
                            find_or_push(
                                additions_for_non_terminal,
                                TransitiveClosureAddition {
                                    item: item.substitute_production(production),
                                    info: follow_set_info.clone(),
                                },
                            );
                        }
                    } else {
                        find_or_push(
                            additions_for_non_terminal,
                            TransitiveClosureAddition {
                                item,
                                info: follow_set_info.clone(),
                            },
                        );
                    }
                }
            }
        }

        result
    }

    pub fn transitive_closure(&self, item_set: &ParseItemSet<'a>) -> ParseItemSet<'a> {
        let mut result = ParseItemSet::default();
        for entry in &item_set.entries {
            if let Some(productions) = self
                .inlines
                .inlined_productions(entry.item.production, entry.item.step_index)
            {
                for production in productions {
                    self.add_item(
                        &mut result,
                        &ParseItemSetEntry {
                            item: entry.item.substitute_production(production),
                            lookaheads: entry.lookaheads.clone(),
                            following_reserved_word_set: entry.following_reserved_word_set,
                        },
                    );
                }
            } else {
                self.add_item(&mut result, entry);
            }
        }
        result
    }

    pub fn first_set(&self, symbol: &Symbol) -> &TokenSet {
        &self.first_sets[symbol]
    }

    pub fn reserved_first_set(&self, symbol: &Symbol) -> Option<&TokenSet> {
        let id = *self.reserved_first_sets.get(symbol)?;
        Some(&self.syntax_grammar.reserved_word_sets[id.0])
    }

    pub fn last_set(&self, symbol: &Symbol) -> &TokenSet {
        &self.last_sets[symbol]
    }

    fn add_item(&self, set: &mut ParseItemSet<'a>, entry: &ParseItemSetEntry<'a>) {
        if let Some(step) = entry.item.step() {
            if step.symbol.is_non_terminal() {
                let next_step = entry.item.successor().step();

                // Determine which tokens can follow this non-terminal.
                let (following_tokens, following_reserved_tokens) =
                    if let Some(next_step) = next_step {
                        (
                            self.first_sets.get(&next_step.symbol).unwrap(),
                            *self.reserved_first_sets.get(&next_step.symbol).unwrap(),
                        )
                    } else {
                        (&entry.lookaheads, entry.following_reserved_word_set)
                    };

                // Use the pre-computed *additions* to expand the non-terminal.
                for addition in &self.transitive_closure_additions[step.symbol.index] {
                    let entry = set.insert(addition.item);
                    entry.lookaheads.insert_all(&addition.info.lookaheads);

                    if let Some(word_token) = self.syntax_grammar.word_token {
                        if addition.info.lookaheads.contains(&word_token) {
                            entry.following_reserved_word_set = entry
                                .following_reserved_word_set
                                .max(addition.info.reserved_lookaheads);
                        }
                    }

                    if addition.info.propagates_lookaheads {
                        entry.lookaheads.insert_all(following_tokens);

                        if let Some(word_token) = self.syntax_grammar.word_token {
                            if following_tokens.contains(&word_token) {
                                entry.following_reserved_word_set = entry
                                    .following_reserved_word_set
                                    .max(following_reserved_tokens);
                            }
                        }
                    }
                }
            }
        }

        let e = set.insert(entry.item);
        e.lookaheads.insert_all(&entry.lookaheads);
        e.following_reserved_word_set = e
            .following_reserved_word_set
            .max(entry.following_reserved_word_set);
    }
}

impl fmt::Debug for ParseItemSetBuilder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "ParseItemSetBuilder {{")?;

        writeln!(f, "  first_sets: {{")?;
        for (symbol, first_set) in &self.first_sets {
            let name = match symbol.kind {
                SymbolType::NonTerminal => &self.syntax_grammar.variables[symbol.index].name,
                SymbolType::External => &self.syntax_grammar.external_tokens[symbol.index].name,
                SymbolType::Terminal => &self.lexical_grammar.variables[symbol.index].name,
                SymbolType::End | SymbolType::EndOfNonTerminalExtra => "END",
            };
            writeln!(
                f,
                "    first({name:?}): {}",
                TokenSetDisplay(first_set, self.syntax_grammar, self.lexical_grammar)
            )?;
        }
        writeln!(f, "  }}")?;

        writeln!(f, "  last_sets: {{")?;
        for (symbol, last_set) in &self.last_sets {
            let name = match symbol.kind {
                SymbolType::NonTerminal => &self.syntax_grammar.variables[symbol.index].name,
                SymbolType::External => &self.syntax_grammar.external_tokens[symbol.index].name,
                SymbolType::Terminal => &self.lexical_grammar.variables[symbol.index].name,
                SymbolType::End | SymbolType::EndOfNonTerminalExtra => "END",
            };
            writeln!(
                f,
                "    last({name:?}): {}",
                TokenSetDisplay(last_set, self.syntax_grammar, self.lexical_grammar)
            )?;
        }
        writeln!(f, "  }}")?;

        writeln!(f, "  additions: {{")?;
        for (i, variable) in self.syntax_grammar.variables.iter().enumerate() {
            writeln!(f, "    {}: {{", variable.name)?;
            for addition in &self.transitive_closure_additions[i] {
                writeln!(
                    f,
                    "      {}",
                    ParseItemDisplay(&addition.item, self.syntax_grammar, self.lexical_grammar)
                )?;
            }
            writeln!(f, "    }},")?;
        }
        write!(f, "  }},")?;

        write!(f, "}}")?;
        Ok(())
    }
}
