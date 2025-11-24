//! LR(1) interpreter. Just builds up parse trees. Intended for testing.

use crate::generate::ParseTree;
use crate::grammar::repr::*;
use crate::lr1::core::*;
use crate::lr1::lookahead::*;
use std::fmt::{Debug, Display, Error, Formatter};
use std::iter::IntoIterator;
use crate::util::Sep;

pub type InterpretError<'grammar, L> = (&'grammar State<'grammar, L>, Token);

/// Feed in the given tokens and then EOF, returning the final parse tree that is reduced.
pub fn interpret<'grammar, L>(
    states: &'grammar [State<'grammar, L>],
    tokens: Vec<TerminalString>,
) -> Result<ParseTree, InterpretError<'grammar, L>>
where
    L: LookaheadInterpret,
{
    println!("interpret(tokens={:?})", tokens);
    let mut m = Machine::new(states);
    m.execute(tokens.into_iter())
}

/// Feed in the given tokens and returns the states on the stack.
pub fn interpret_partial<'grammar, TOKENS, L>(
    states: &'grammar [State<'grammar, L>],
    tokens: TOKENS,
) -> Result<Vec<StateIndex>, InterpretError<'grammar, L>>
where
    TOKENS: IntoIterator<Item = TerminalString>,
    L: LookaheadInterpret,
{
    let mut m = Machine::new(states);
    m.execute_partial(tokens.into_iter())?;
    Ok(m.state_stack)
}

struct Machine<'grammar, L: LookaheadInterpret + 'grammar> {
    states: &'grammar [State<'grammar, L>],
    state_stack: Vec<StateIndex>,
    data_stack: Vec<ParseTree>,
}

impl<'grammar, L> Machine<'grammar, L>
where
    L: LookaheadInterpret,
{
    fn new(states: &'grammar [State<'grammar, L>]) -> Machine<'grammar, L> {
        Machine {
            states,
            state_stack: vec![],
            data_stack: vec![],
        }
    }

    fn top_state(&self) -> &'grammar State<'grammar, L> {
        let index = self.state_stack.last().unwrap();
        &self.states[index.0]
    }

    fn execute_partial<TOKENS>(
        &mut self,
        mut tokens: TOKENS,
    ) -> Result<(), InterpretError<'grammar, L>>
    where
        TOKENS: Iterator<Item = TerminalString>,
    {
        assert!(self.state_stack.is_empty());
        assert!(self.data_stack.is_empty());

        self.state_stack.push(StateIndex(0));

        let mut token = tokens.next();
        while let Some(terminal) = token.clone() {
            let state = self.top_state();

            println!("state={:?}", state);
            println!("terminal={:?}", terminal);

            // check whether we can shift this token
            if let Some(&next_index) = state.shifts.get(&terminal) {
                self.data_stack.push(ParseTree::Terminal(terminal.clone()));
                self.state_stack.push(next_index);
                token = tokens.next();
            } else if let Some(production) = L::reduction(state, &Token::Terminal(terminal.clone()))
            {
                let more = self.reduce(production);
                assert!(more);
            } else {
                return Err((state, Token::Terminal(terminal.clone())));
            }
        }

        Ok(())
    }

    fn execute<TOKENS>(&mut self, tokens: TOKENS) -> Result<ParseTree, InterpretError<'grammar, L>>
    where
        TOKENS: Iterator<Item = TerminalString>,
    {
        self.execute_partial(tokens)?;

        // drain now for EOF
        loop {
            let state = self.top_state();
            match L::reduction(state, &Token::EOF) {
                None => {
                    return Err((state, Token::EOF));
                }
                Some(production) => {
                    if !self.reduce(production) {
                        assert_eq!(self.data_stack.len(), 1);
                        return Ok(self.data_stack.pop().unwrap());
                    }
                }
            }
        }
    }

    fn reduce(&mut self, production: &Production) -> bool {
        println!("reduce={:?}", production);

        let args = production.symbols.len();

        // remove the top N items from the data stack
        let mut popped = vec![];
        for _ in 0..args {
            popped.push(self.data_stack.pop().unwrap());
        }
        popped.reverse();

        // remove the top N states
        for _ in 0..args {
            self.state_stack.pop().unwrap();
        }

        // construct the new, reduced tree and push it on the stack
        let tree = ParseTree::Nonterminal(production.nonterminal.clone(), popped);
        self.data_stack.push(tree);

        // recover the state and extract the "Goto" action
        let receiving_state = self.top_state();
        match receiving_state.gotos.get(&production.nonterminal) {
            Some(&goto_state) => {
                self.state_stack.push(goto_state);
                true // keep going
            }
            None => {
                false // all done
            }
        }
    }
}

impl Debug for ParseTree {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        Display::fmt(self, fmt)
    }
}

impl Display for ParseTree {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            ParseTree::Nonterminal(ref id, ref trees) => {
                write!(fmt, "[{}: {}]", id, Sep(", ", trees))
            }
            ParseTree::Terminal(ref id) => write!(fmt, "{}", id),
        }
    }
}

pub trait LookaheadInterpret: Lookahead {
    fn reduction<'grammar>(
        state: &State<'grammar, Self>,
        token: &Token,
    ) -> Option<&'grammar Production>;
}

impl LookaheadInterpret for Nil {
    fn reduction<'grammar>(
        state: &State<'grammar, Self>,
        _token: &Token,
    ) -> Option<&'grammar Production> {
        state
            .reductions
            .iter()
            .map(|&(_, production)| production)
            .next()
    }
}

impl LookaheadInterpret for TokenSet {
    fn reduction<'grammar>(
        state: &State<'grammar, Self>,
        token: &Token,
    ) -> Option<&'grammar Production> {
        state
            .reductions
            .iter()
            .filter(|&&(ref tokens, _)| tokens.contains(token))
            .map(|&(_, production)| production)
            .next()
    }
}
