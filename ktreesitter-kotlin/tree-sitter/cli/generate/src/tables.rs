use std::collections::BTreeMap;

use super::{
    nfa::CharacterSet,
    rules::{Alias, Symbol, TokenSet},
};
pub type ProductionInfoId = usize;
pub type ParseStateId = usize;
pub type LexStateId = usize;

use std::hash::BuildHasherDefault;

use indexmap::IndexMap;
use rustc_hash::FxHasher;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParseAction {
    Accept,
    Shift {
        state: ParseStateId,
        is_repetition: bool,
    },
    ShiftExtra,
    Recover,
    Reduce {
        symbol: Symbol,
        child_count: usize,
        dynamic_precedence: i32,
        production_id: ProductionInfoId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GotoAction {
    Goto(ParseStateId),
    ShiftExtra,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParseTableEntry {
    pub actions: Vec<ParseAction>,
    pub reusable: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParseState {
    pub id: ParseStateId,
    pub terminal_entries: IndexMap<Symbol, ParseTableEntry, BuildHasherDefault<FxHasher>>,
    pub nonterminal_entries: IndexMap<Symbol, GotoAction, BuildHasherDefault<FxHasher>>,
    pub reserved_words: TokenSet,
    pub lex_state_id: usize,
    pub external_lex_state_id: usize,
    pub core_id: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FieldLocation {
    pub index: usize,
    pub inherited: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ProductionInfo {
    pub alias_sequence: Vec<Option<Alias>>,
    pub field_map: BTreeMap<String, Vec<FieldLocation>>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ParseTable {
    pub states: Vec<ParseState>,
    pub symbols: Vec<Symbol>,
    pub production_infos: Vec<ProductionInfo>,
    pub max_aliased_production_length: usize,
    pub external_lex_states: Vec<TokenSet>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AdvanceAction {
    pub state: LexStateId,
    pub in_main_token: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct LexState {
    pub accept_action: Option<Symbol>,
    pub eof_action: Option<AdvanceAction>,
    pub advance_actions: Vec<(CharacterSet, AdvanceAction)>,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub struct LexTable {
    pub states: Vec<LexState>,
}

impl ParseTableEntry {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reusable: true,
            actions: Vec::new(),
        }
    }
}

impl ParseState {
    pub fn is_end_of_non_terminal_extra(&self) -> bool {
        self.terminal_entries
            .contains_key(&Symbol::end_of_nonterminal_extra())
    }

    pub fn referenced_states(&self) -> impl Iterator<Item = ParseStateId> + '_ {
        self.terminal_entries
            .iter()
            .flat_map(|(_, entry)| {
                entry.actions.iter().filter_map(|action| match action {
                    ParseAction::Shift { state, .. } => Some(*state),
                    _ => None,
                })
            })
            .chain(self.nonterminal_entries.iter().filter_map(|(_, action)| {
                if let GotoAction::Goto(state) = action {
                    Some(*state)
                } else {
                    None
                }
            }))
    }

    pub fn update_referenced_states<F>(&mut self, mut f: F)
    where
        F: FnMut(usize, &Self) -> usize,
    {
        let mut updates = Vec::new();
        for (symbol, entry) in &self.terminal_entries {
            for (i, action) in entry.actions.iter().enumerate() {
                if let ParseAction::Shift { state, .. } = action {
                    let result = f(*state, self);
                    if result != *state {
                        updates.push((*symbol, i, result));
                    }
                }
            }
        }
        for (symbol, action) in &self.nonterminal_entries {
            if let GotoAction::Goto(other_state) = action {
                let result = f(*other_state, self);
                if result != *other_state {
                    updates.push((*symbol, 0, result));
                }
            }
        }
        for (symbol, action_index, new_state) in updates {
            if symbol.is_non_terminal() {
                self.nonterminal_entries
                    .insert(symbol, GotoAction::Goto(new_state));
            } else {
                let entry = self.terminal_entries.get_mut(&symbol).unwrap();
                if let ParseAction::Shift { is_repetition, .. } = entry.actions[action_index] {
                    entry.actions[action_index] = ParseAction::Shift {
                        state: new_state,
                        is_repetition,
                    };
                }
            }
        }
    }
}
