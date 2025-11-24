use bit_set::{self, BitSet};
use crate::collections::Collection;
use crate::grammar::repr::*;
use crate::lr1::core::*;
use crate::lr1::tls::Lr1Tls;
use std::fmt::{Debug, Error, Formatter};
use std::hash::Hash;

pub trait Lookahead: Clone + Debug + Eq + Ord + Hash + Collection<Item = Self> {
    fn fmt_as_item_suffix(&self, fmt: &mut Formatter) -> Result<(), Error>;

    fn conflicts<'grammar>(this_state: &State<'grammar, Self>) -> Vec<Conflict<'grammar, Self>>;
}

#[derive(Copy, Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Nil;

impl Collection for Nil {
    type Item = Nil;

    fn push(&mut self, _: Nil) -> bool {
        false
    }
}

impl Lookahead for Nil {
    fn fmt_as_item_suffix(&self, _fmt: &mut Formatter) -> Result<(), Error> {
        Ok(())
    }

    fn conflicts<'grammar>(this_state: &State<'grammar, Self>) -> Vec<Conflict<'grammar, Self>> {
        let index = this_state.index;

        let mut conflicts = vec![];

        for (terminal, &next_state) in &this_state.shifts {
            conflicts.extend(
                this_state
                    .reductions
                    .iter()
                    .map(|&(_, production)| Conflict {
                        state: index,
                        lookahead: Nil,
                        production,
                        action: Action::Shift(terminal.clone(), next_state),
                    }),
            );
        }

        if this_state.reductions.len() > 1 {
            for &(_, production) in &this_state.reductions[1..] {
                let other_production = this_state.reductions[0].1;
                conflicts.push(Conflict {
                    state: index,
                    lookahead: Nil,
                    production,
                    action: Action::Reduce(other_production),
                });
            }
        }

        conflicts
    }
}

/// I have semi-arbitrarily decided to use the term "token" to mean
/// either one of the terminals of our language, or else the
/// pseudo-symbol EOF that represents "end of input".
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Token {
    EOF,
    Error,
    Terminal(TerminalString),
}

impl Lookahead for TokenSet {
    fn fmt_as_item_suffix(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, " {:?}", self)
    }

    fn conflicts<'grammar>(this_state: &State<'grammar, Self>) -> Vec<Conflict<'grammar, Self>> {
        let mut conflicts = vec![];

        for (terminal, &next_state) in &this_state.shifts {
            let token = Token::Terminal(terminal.clone());
            let inconsistent =
                this_state
                    .reductions
                    .iter()
                    .filter_map(|&(ref reduce_tokens, production)| {
                        if reduce_tokens.contains(&token) {
                            Some(production)
                        } else {
                            None
                        }
                    });
            let set = TokenSet::from(token.clone());
            for production in inconsistent {
                conflicts.push(Conflict {
                    state: this_state.index,
                    lookahead: set.clone(),
                    production,
                    action: Action::Shift(terminal.clone(), next_state),
                });
            }
        }

        let len = this_state.reductions.len();
        for i in 0..len {
            for j in i + 1..len {
                let &(ref i_tokens, i_production) = &this_state.reductions[i];
                let &(ref j_tokens, j_production) = &this_state.reductions[j];

                if i_tokens.is_disjoint(j_tokens) {
                    continue;
                }

                conflicts.push(Conflict {
                    state: this_state.index,
                    lookahead: i_tokens.intersection(j_tokens),
                    production: i_production,
                    action: Action::Reduce(j_production),
                });
            }
        }

        conflicts
    }
}

impl Token {
    pub fn unwrap_terminal(&self) -> &TerminalString {
        match *self {
            Token::Terminal(ref t) => t,
            Token::EOF | Token::Error => {
                panic!("`unwrap_terminal()` invoked but with EOF or Error")
            }
        }
    }
}

#[derive(Clone, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TokenSet {
    bit_set: BitSet<u32>,
}

fn with<OP, RET>(op: OP) -> RET
where
    OP: FnOnce(&TerminalSet) -> RET,
{
    Lr1Tls::with(op)
}

impl TokenSet {
    pub fn new() -> Self {
        with(|terminals| TokenSet {
            bit_set: BitSet::with_capacity(terminals.all.len() + 2),
        })
    }

    /// A TokenSet containing all possible terminals + EOF.
    pub fn all() -> Self {
        let mut s = TokenSet::new();
        with(|terminals| {
            for i in 0..terminals.all.len() {
                s.bit_set.insert(i);
            }
            s.insert_eof();
        });
        s
    }

    pub fn eof() -> Self {
        let mut set = TokenSet::new();
        set.insert_eof();
        set
    }

    fn eof_bit(&self) -> usize {
        with(|terminals| terminals.all.len())
    }

    fn bit(&self, lookahead: &Token) -> usize {
        match *lookahead {
            Token::EOF => self.eof_bit(),
            Token::Error => self.eof_bit() + 1,
            Token::Terminal(ref t) => with(|terminals| terminals.bits[t]),
        }
    }

    pub fn len(&self) -> usize {
        self.bit_set.len()
    }

    pub fn insert(&mut self, lookahead: Token) -> bool {
        let bit = self.bit(&lookahead);
        self.bit_set.insert(bit)
    }

    pub fn insert_eof(&mut self) -> bool {
        let bit = self.eof_bit();
        self.bit_set.insert(bit)
    }

    pub fn union_with(&mut self, set: &TokenSet) -> bool {
        let len = self.len();
        self.bit_set.union_with(&set.bit_set);
        self.len() != len
    }

    pub fn intersection(&self, set: &TokenSet) -> TokenSet {
        let mut bit_set = self.bit_set.clone();
        bit_set.intersect_with(&set.bit_set);
        TokenSet { bit_set }
    }

    pub fn contains(&self, token: &Token) -> bool {
        self.bit_set.contains(self.bit(token))
    }

    pub fn contains_eof(&self) -> bool {
        self.bit_set.contains(self.eof_bit())
    }

    /// If this set contains EOF, removes it from the set and returns
    /// true. Otherwise, returns false.
    pub fn take_eof(&mut self) -> bool {
        let eof_bit = self.eof_bit();
        let contains_eof = self.bit_set.contains(eof_bit);
        self.bit_set.remove(eof_bit);
        contains_eof
    }

    pub fn is_disjoint(&self, other: &TokenSet) -> bool {
        self.bit_set.is_disjoint(&other.bit_set)
    }

    pub fn is_intersecting(&self, other: &TokenSet) -> bool {
        !self.is_disjoint(other)
    }

    pub fn iter(&self) -> TokenSetIter<'_> {
        TokenSetIter {
            bit_set: self.bit_set.iter(),
        }
    }
}

pub struct TokenSetIter<'iter> {
    bit_set: bit_set::Iter<'iter, u32>,
}

impl<'iter> Iterator for TokenSetIter<'iter> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        self.bit_set.next().map(|bit| {
            with(|terminals| {
                if bit == terminals.all.len() + 1 {
                    Token::Error
                } else if bit == terminals.all.len() {
                    Token::EOF
                } else {
                    Token::Terminal(terminals.all[bit].clone())
                }
            })
        })
    }
}

impl<'debug> Debug for TokenSet {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        let terminals: Vec<_> = self.iter().collect();
        Debug::fmt(&terminals, fmt)
    }
}

impl<'iter> IntoIterator for &'iter TokenSet {
    type IntoIter = TokenSetIter<'iter>;
    type Item = Token;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl Collection for TokenSet {
    type Item = TokenSet;

    fn push(&mut self, item: TokenSet) -> bool {
        self.union_with(&item)
    }
}

impl From<Token> for TokenSet {
    fn from(token: Token) -> Self {
        let mut set = TokenSet::new();
        set.insert(token);
        set
    }
}
