//! Core LR(1) types.

use crate::collections::Map;
use crate::grammar::repr::*;
use crate::util::Prefix;
use itertools::Itertools;
use std::fmt::{Debug, Display, Error, Formatter};

use super::lookahead::*;

#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Item<'grammar, L: Lookahead> {
    pub production: &'grammar Production,
    /// the dot comes before `index`, so `index` would be 1 for X = A (*) B C
    pub index: usize,
    pub lookahead: L,
}

pub type LR0Item<'grammar> = Item<'grammar, Nil>;

pub type LR1Item<'grammar> = Item<'grammar, TokenSet>;

impl<'grammar> Item<'grammar, Nil> {
    pub fn lr0(production: &'grammar Production, index: usize) -> Self {
        Item {
            production,
            index,
            lookahead: Nil,
        }
    }
}

impl<'grammar, L: Lookahead> Item<'grammar, L> {
    pub fn with_lookahead<L1: Lookahead>(&self, l: L1) -> Item<'grammar, L1> {
        Item {
            production: self.production,
            index: self.index,
            lookahead: l,
        }
    }

    pub fn prefix(&self) -> &'grammar [Symbol] {
        &self.production.symbols[..self.index]
    }

    pub fn symbol_sets(&self) -> SymbolSets<'grammar> {
        let symbols = &self.production.symbols;
        if self.can_shift() {
            SymbolSets {
                prefix: &symbols[..self.index],
                cursor: Some(&symbols[self.index]),
                suffix: &symbols[self.index + 1..],
            }
        } else {
            SymbolSets {
                prefix: &symbols[..self.index],
                cursor: None,
                suffix: &[],
            }
        }
    }

    pub fn to_lr0(&self) -> LR0Item<'grammar> {
        Item {
            production: self.production,
            index: self.index,
            lookahead: Nil,
        }
    }

    pub fn can_shift(&self) -> bool {
        self.index < self.production.symbols.len()
    }

    pub fn can_shift_nonterminal(&self, nt: &NonterminalString) -> bool {
        match self.shift_symbol() {
            Some((Symbol::Nonterminal(shifted), _)) => shifted == *nt,
            _ => false,
        }
    }

    pub fn can_shift_terminal(&self, term: &TerminalString) -> bool {
        match self.shift_symbol() {
            Some((Symbol::Terminal(shifted), _)) => shifted == *term,
            _ => false,
        }
    }

    pub fn can_reduce(&self) -> bool {
        self.index == self.production.symbols.len()
    }

    pub fn shifted_item(&self) -> Option<(Symbol, Item<'grammar, L>)> {
        if self.can_shift() {
            Some((
                self.production.symbols[self.index].clone(),
                Item {
                    production: self.production,
                    index: self.index + 1,
                    lookahead: self.lookahead.clone(),
                },
            ))
        } else {
            None
        }
    }

    pub fn shift_symbol(&self) -> Option<(Symbol, &[Symbol])> {
        if self.can_shift() {
            Some((
                self.production.symbols[self.index].clone(),
                &self.production.symbols[self.index + 1..],
            ))
        } else {
            None
        }
    }
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct StateIndex(pub usize);

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Items<'grammar, L: Lookahead> {
    pub vec: Vec<Item<'grammar, L>>,
}

#[allow(dead_code)]
pub type LR0Items<'grammar> = Items<'grammar, Nil>;
#[allow(dead_code)]
pub type LR1Items<'grammar> = Items<'grammar, TokenSet>;

#[derive(Clone, Debug)]
pub struct State<'grammar, L: Lookahead> {
    pub index: StateIndex,
    pub items: Items<'grammar, L>,
    pub shifts: Map<TerminalString, StateIndex>,
    pub reductions: Vec<(L, &'grammar Production)>,
    pub gotos: Map<NonterminalString, StateIndex>,
}

pub type LR0State<'grammar> = State<'grammar, Nil>;
pub type LR1State<'grammar> = State<'grammar, TokenSet>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action<'grammar> {
    Shift(TerminalString, StateIndex),
    Reduce(&'grammar Production),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict<'grammar, L> {
    // when in this state...
    pub state: StateIndex,

    // with the following lookahead...
    pub lookahead: L,

    // we can reduce...
    pub production: &'grammar Production,

    // but we can also...
    pub action: Action<'grammar>,
}

#[allow(dead_code)]
pub type LR0Conflict<'grammar> = Conflict<'grammar, Nil>;
pub type LR1Conflict<'grammar> = Conflict<'grammar, TokenSet>;

#[derive(Debug)]
pub struct TableConstructionError<'grammar, L: Lookahead> {
    // LR(1) state set, possibly incomplete if construction is
    // configured to terminate early.
    pub states: Vec<State<'grammar, L>>,

    // Conflicts (non-empty) found in those states.
    pub conflicts: Vec<Conflict<'grammar, L>>,
}

pub type LR0TableConstructionError<'grammar> = TableConstructionError<'grammar, Nil>;
pub type LR1TableConstructionError<'grammar> = TableConstructionError<'grammar, TokenSet>;
pub type LRResult<'grammar, L> =
    Result<Vec<State<'grammar, L>>, TableConstructionError<'grammar, L>>;
pub type LR1Result<'grammar> = LRResult<'grammar, TokenSet>;

impl<'grammar, L: Lookahead> Debug for Item<'grammar, L> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(
            fmt,
            "{} ={} (*){}",
            self.production.nonterminal,
            Prefix(" ", &self.production.symbols[..self.index]),
            Prefix(" ", &self.production.symbols[self.index..])
        )?;

        self.lookahead.fmt_as_item_suffix(fmt)
    }
}

impl Display for Token {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            Token::EOF => write!(fmt, "EOF"),
            Token::Error => write!(fmt, "Error"),
            Token::Terminal(ref s) => write!(fmt, "{}", s),
        }
    }
}

impl Debug for Token {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "{}", self)
    }
}

impl Debug for StateIndex {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "S{}", self.0)
    }
}

impl Display for StateIndex {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "{}", self.0)
    }
}

impl<'grammar, L: Lookahead> State<'grammar, L> {
    /// Returns the set of symbols which must appear on the stack to
    /// be in this state. This is the *maximum* prefix of any item,
    /// basically.
    pub fn max_prefix(&self) -> &'grammar [Symbol] {
        // Each state fn takes as argument the longest prefix of any
        // item. Note that all items must have compatible prefixes.
        let prefix = self
            .items
            .vec
            .iter()
            .map(Item::prefix)
            .max_by_key(|symbols| symbols.len())
            .unwrap();

        debug_assert!(self
            .items
            .vec
            .iter()
            .all(|item| prefix.ends_with(&item.production.symbols[..item.index])));

        prefix
    }

    /// Returns the set of symbols from the stack that must be popped
    /// for this state to return. If we have a state like:
    ///
    /// ```
    /// X = A B C (*) C
    /// Y = B C (*) C
    /// C = (*) ...
    /// ```
    ///
    /// This would return `[B, C]`. For every state other than the
    /// start state, this will return a list of length at least 1.
    /// For the start state, returns `[]`.
    pub fn will_pop(&self) -> &'grammar [Symbol] {
        let prefix = self
            .items
            .vec
            .iter()
            .filter(|item| item.index > 0)
            .map(Item::prefix)
            .min_by_key(|symbols| symbols.len())
            .unwrap_or(&[]);

        debug_assert!(self
            .items
            .vec
            .iter()
            .filter(|item| item.index > 0)
            .all(|item| item.prefix().ends_with(prefix)));

        prefix
    }

    pub fn will_push(&self) -> &[Symbol] {
        self.items
            .vec
            .iter()
            .filter(|item| item.index > 0)
            .map(|item| &item.production.symbols[item.index..])
            .min_by_key(|symbols| symbols.len())
            .unwrap_or(&[])
    }

    /// Returns the type of nonterminal that this state will produce;
    /// if `None` is returned, then this state may produce more than
    /// one kind of nonterminal.
    ///
    /// FIXME -- currently, the start state returns `None` instead of
    /// the goal symbol.
    pub fn will_produce(&self) -> Option<NonterminalString> {
        let mut returnable_nonterminals: Vec<_> = self
            .items
            .vec
            .iter()
            .filter(|item| item.index > 0)
            .map(|item| item.production.nonterminal.clone())
            .dedup()
            .collect();
        if returnable_nonterminals.len() == 1 {
            returnable_nonterminals.pop()
        } else {
            None
        }
    }
}

/// `A = B C (*) D E F` or `A = B C (*)`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SymbolSets<'grammar> {
    pub prefix: &'grammar [Symbol],       // both cases, [B, C]
    pub cursor: Option<&'grammar Symbol>, // first [D], second []
    pub suffix: &'grammar [Symbol],       // first [E, F], second []
}

impl<'grammar> SymbolSets<'grammar> {
    pub fn new() -> Self {
        SymbolSets {
            prefix: &[],
            cursor: None,
            suffix: &[],
        }
    }
}
