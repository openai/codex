//! First set construction and computation.

use crate::collections::{map, Map};
use crate::grammar::repr::*;
use crate::lr1::lookahead::{Token, TokenSet};

#[cfg(test)]
mod test;

#[derive(Clone)]
pub struct FirstSets {
    map: Map<NonterminalString, TokenSet>,
}

impl FirstSets {
    pub fn new(grammar: &Grammar) -> FirstSets {
        let mut this = FirstSets { map: map() };
        let mut changed = true;
        while changed {
            changed = false;
            for production in grammar.nonterminals.values().flat_map(|p| &p.productions) {
                let nt = &production.nonterminal;
                let lookahead = this.first0(&production.symbols);
                let first_set = this.map.entry(nt.clone()).or_insert_with(TokenSet::new);
                changed |= first_set.union_with(&lookahead);
            }
        }
        this
    }

    /// Returns `FIRST(...symbols)`. If `...symbols` may derive
    /// epsilon, then this returned set will include EOF. (This is
    /// kind of repurposing EOF to serve as a binary flag of sorts.)
    pub fn first0<'s, I>(&self, symbols: I) -> TokenSet
    where
        I: IntoIterator<Item = &'s Symbol>,
    {
        let mut result = TokenSet::new();

        for symbol in symbols {
            match *symbol {
                Symbol::Terminal(ref t) => {
                    result.insert(Token::Terminal(t.clone()));
                    return result;
                }

                Symbol::Nonterminal(ref nt) => {
                    let mut empty_prod = false;
                    match self.map.get(nt) {
                        None => {
                            // This should only happen during set
                            // construction; it corresponds to an
                            // entry that has not yet been
                            // built. Otherwise, it would mean a
                            // terminal with no productions. Either
                            // way, the resulting first set should be
                            // empty.
                        }
                        Some(set) => {
                            for lookahead in set.iter() {
                                match lookahead {
                                    Token::EOF => {
                                        empty_prod = true;
                                    }
                                    Token::Error | Token::Terminal(_) => {
                                        result.insert(lookahead);
                                    }
                                }
                            }
                        }
                    }
                    if !empty_prod {
                        return result;
                    }
                }
            }
        }

        // control only reaches here if either symbols is empty, or it
        // consists of nonterminals all of which may derive epsilon
        result.insert(Token::EOF);
        result
    }

    pub fn first1(&self, symbols: &[Symbol], lookahead: &TokenSet) -> TokenSet {
        let mut set = self.first0(symbols);

        // we use EOF as the signal that `symbols` derives epsilon:
        let epsilon = set.take_eof();

        if epsilon {
            set.union_with(lookahead);
        }

        set
    }
}
