//! Generate valid parse trees.

use crate::grammar::repr::*;
use rand::{self, Rng};
use std::iter::Iterator;

#[derive(PartialEq, Eq)]
pub enum ParseTree {
    Nonterminal(NonterminalString, Vec<ParseTree>),
    Terminal(TerminalString),
}

pub fn random_parse_tree(grammar: &Grammar, symbol: NonterminalString) -> ParseTree {
    let mut gen = Generator {
        grammar,
        rng: rand::thread_rng(),
        depth: 0,
    };
    loop {
        // sometimes, the random walk overflows the stack, so we have a max, and if
        // it is exceeded, we just try again
        if let Some(result) = gen.nonterminal(symbol.clone()) {
            return result;
        }
        gen.depth = 0;
    }
}

struct Generator<'grammar> {
    grammar: &'grammar Grammar,
    rng: rand::rngs::ThreadRng,
    depth: u32,
}

const MAX_DEPTH: u32 = 7000;

impl<'grammar> Generator<'grammar> {
    fn nonterminal(&mut self, nt: NonterminalString) -> Option<ParseTree> {
        if self.depth > MAX_DEPTH {
            return None;
        }

        self.depth += 1;
        let productions = self.grammar.productions_for(&nt);
        let index: usize = self.rng.gen_range(0..productions.len());
        let production = &productions[index];
        let trees: Option<Vec<_>> = production
            .symbols
            .iter()
            .map(|sym| self.symbol(sym.clone()))
            .collect();
        trees.map(|trees| ParseTree::Nonterminal(nt, trees))
    }

    fn symbol(&mut self, symbol: Symbol) -> Option<ParseTree> {
        match symbol {
            Symbol::Nonterminal(nt) => self.nonterminal(nt),
            Symbol::Terminal(t) => Some(ParseTree::Terminal(t)),
        }
    }
}

impl ParseTree {
    pub fn terminals(&self) -> Vec<TerminalString> {
        let mut vec = vec![];
        self.push_terminals(&mut vec);
        vec
    }

    fn push_terminals(&self, vec: &mut Vec<TerminalString>) {
        match *self {
            ParseTree::Terminal(ref s) => vec.push(s.clone()),
            ParseTree::Nonterminal(_, ref trees) => {
                for tree in trees {
                    tree.push_terminals(vec);
                }
            }
        }
    }
}
