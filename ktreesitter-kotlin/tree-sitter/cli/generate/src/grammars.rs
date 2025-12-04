use std::{collections::HashMap, fmt};

use super::{
    nfa::Nfa,
    rules::{Alias, Associativity, Precedence, Rule, Symbol, TokenSet},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum VariableType {
    Hidden,
    Auxiliary,
    Anonymous,
    Named,
}

// Input grammar

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    pub kind: VariableType,
    pub rule: Rule,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrecedenceEntry {
    Name(String),
    Symbol(String),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct InputGrammar {
    pub name: String,
    pub variables: Vec<Variable>,
    pub extra_symbols: Vec<Rule>,
    pub expected_conflicts: Vec<Vec<String>>,
    pub precedence_orderings: Vec<Vec<PrecedenceEntry>>,
    pub external_tokens: Vec<Rule>,
    pub variables_to_inline: Vec<String>,
    pub supertype_symbols: Vec<String>,
    pub word_token: Option<String>,
    pub reserved_words: Vec<ReservedWordContext<Rule>>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReservedWordContext<T> {
    pub name: String,
    pub reserved_words: Vec<T>,
}

// Extracted lexical grammar

#[derive(Debug, PartialEq, Eq)]
pub struct LexicalVariable {
    pub name: String,
    pub kind: VariableType,
    pub implicit_precedence: i32,
    pub start_state: u32,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct LexicalGrammar {
    pub nfa: Nfa,
    pub variables: Vec<LexicalVariable>,
}

// Extracted syntax grammar

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProductionStep {
    pub symbol: Symbol,
    pub precedence: Precedence,
    pub associativity: Option<Associativity>,
    pub alias: Option<Alias>,
    pub field_name: Option<String>,
    pub reserved_word_set_id: ReservedWordSetId,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReservedWordSetId(pub usize);

impl fmt::Display for ReservedWordSetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub const NO_RESERVED_WORDS: ReservedWordSetId = ReservedWordSetId(usize::MAX);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Production {
    pub steps: Vec<ProductionStep>,
    pub dynamic_precedence: i32,
}

#[derive(Default)]
pub struct InlinedProductionMap {
    pub productions: Vec<Production>,
    pub production_map: HashMap<(*const Production, u32), Vec<usize>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxVariable {
    pub name: String,
    pub kind: VariableType,
    pub productions: Vec<Production>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalToken {
    pub name: String,
    pub kind: VariableType,
    pub corresponding_internal_token: Option<Symbol>,
}

#[derive(Debug, Default)]
pub struct SyntaxGrammar {
    pub variables: Vec<SyntaxVariable>,
    pub extra_symbols: Vec<Symbol>,
    pub expected_conflicts: Vec<Vec<Symbol>>,
    pub external_tokens: Vec<ExternalToken>,
    pub supertype_symbols: Vec<Symbol>,
    pub variables_to_inline: Vec<Symbol>,
    pub word_token: Option<Symbol>,
    pub precedence_orderings: Vec<Vec<PrecedenceEntry>>,
    pub reserved_word_sets: Vec<TokenSet>,
}

#[cfg(test)]
impl ProductionStep {
    #[must_use]
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            precedence: Precedence::None,
            associativity: None,
            alias: None,
            field_name: None,
            reserved_word_set_id: ReservedWordSetId::default(),
        }
    }

    pub fn with_prec(
        mut self,
        precedence: Precedence,
        associativity: Option<Associativity>,
    ) -> Self {
        self.precedence = precedence;
        self.associativity = associativity;
        self
    }

    pub fn with_alias(mut self, value: &str, is_named: bool) -> Self {
        self.alias = Some(Alias {
            value: value.to_string(),
            is_named,
        });
        self
    }

    pub fn with_field_name(mut self, name: &str) -> Self {
        self.field_name = Some(name.to_string());
        self
    }
}

impl Production {
    pub fn first_symbol(&self) -> Option<Symbol> {
        self.steps.first().map(|s| s.symbol)
    }
}

#[cfg(test)]
impl Variable {
    pub fn named(name: &str, rule: Rule) -> Self {
        Self {
            name: name.to_string(),
            kind: VariableType::Named,
            rule,
        }
    }

    pub fn auxiliary(name: &str, rule: Rule) -> Self {
        Self {
            name: name.to_string(),
            kind: VariableType::Auxiliary,
            rule,
        }
    }

    pub fn hidden(name: &str, rule: Rule) -> Self {
        Self {
            name: name.to_string(),
            kind: VariableType::Hidden,
            rule,
        }
    }

    pub fn anonymous(name: &str, rule: Rule) -> Self {
        Self {
            name: name.to_string(),
            kind: VariableType::Anonymous,
            rule,
        }
    }
}

impl VariableType {
    pub fn is_visible(self) -> bool {
        self == Self::Named || self == Self::Anonymous
    }
}

impl LexicalGrammar {
    pub fn variable_indices_for_nfa_states<'a>(
        &'a self,
        state_ids: &'a [u32],
    ) -> impl Iterator<Item = usize> + 'a {
        let mut prev = None;
        state_ids.iter().filter_map(move |state_id| {
            let variable_id = self.variable_index_for_nfa_state(*state_id);
            if prev == Some(variable_id) {
                None
            } else {
                prev = Some(variable_id);
                prev
            }
        })
    }

    pub fn variable_index_for_nfa_state(&self, state_id: u32) -> usize {
        self.variables
            .iter()
            .position(|v| v.start_state >= state_id)
            .unwrap()
    }
}

impl SyntaxVariable {
    pub fn is_auxiliary(&self) -> bool {
        self.kind == VariableType::Auxiliary
    }

    pub fn is_hidden(&self) -> bool {
        self.kind == VariableType::Hidden || self.kind == VariableType::Auxiliary
    }
}

impl InlinedProductionMap {
    pub fn inlined_productions<'a>(
        &'a self,
        production: &Production,
        step_index: u32,
    ) -> Option<impl Iterator<Item = &'a Production> + 'a> {
        self.production_map
            .get(&(std::ptr::from_ref::<Production>(production), step_index))
            .map(|production_indices| {
                production_indices
                    .iter()
                    .copied()
                    .map(move |index| &self.productions[index])
            })
    }
}

impl fmt::Display for PrecedenceEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Name(n) => write!(f, "'{n}'"),
            Self::Symbol(s) => write!(f, "$.{s}"),
        }
    }
}
