#![allow(dead_code)]

use alloc::{string::String, vec, vec::Vec};
use core::fmt::Debug;

#[cfg(feature = "std")]
const DEBUG_ENABLED: bool = false;

macro_rules! debug {
    ($($args:expr),* $(,)*) => {
        #[cfg(feature = "std")]
        if DEBUG_ENABLED {
            eprintln!($($args),*);
        }
    }
}

pub trait ParserDefinition: Sized {
    /// Represents a location in the input text. If you are using the
    /// default tokenizer, this will be a `usize`.
    type Location: Clone + Debug;

    /// Represents a "user error" -- this can get produced by
    /// `reduce()` if the grammar includes `=>?` actions.
    type Error;

    /// The type emitted by the user's tokenizer (excluding the
    /// location information).
    type Token: Clone + Debug;

    /// We assign a unique index to each token in the grammar, which
    /// we call its *index*. When we pull in a new `Token` from the
    /// input, we then match against it to determine its index.  Note
    /// that the actual `Token` is retained too, as it may carry
    /// additional information (e.g., an `ID` terminal often has a
    /// string value associated with it; this is not important to the
    /// parser, but the semantic analyzer will want it).
    type TokenIndex: Copy + Clone + Debug;

    /// The type representing things on the LALRPOP stack. Represents
    /// the union of terminals and nonterminals.
    type Symbol;

    /// Type produced by reducing the start symbol.
    type Success;

    /// Identifies a state. Typically an i8, i16, or i32 (depending on
    /// how many states you have).
    type StateIndex: Copy + Clone + Debug;

    /// Identifies an action.
    type Action: ParserAction<Self>;

    /// Identifies a reduction.
    type ReduceIndex: Copy + Clone + Debug;

    /// Identifies a nonterminal.
    type NonterminalIndex: Copy + Clone + Debug;

    /// Returns a location representing the "start of the input".
    fn start_location(&self) -> Self::Location;

    /// Returns the initial state.
    fn start_state(&self) -> Self::StateIndex;

    /// Converts the user's tokens into an internal index; this index
    /// is then used to index into actions and the like. When using an
    /// internal tokenizer, these indices are directly produced. When
    /// using an **external** tokenier, however, this function matches
    /// against the patterns given by the user: it is fallible
    /// therefore as these patterns may not be exhaustive. If a token
    /// value is found that doesn't match any of the patterns the user
    /// supplied, then this function returns `None`, which is
    /// translated into a parse error by LALRPOP ("unrecognized
    /// token").
    fn token_to_index(&self, token: &Self::Token) -> Option<Self::TokenIndex>;

    /// Given the top-most state and the pending terminal, returns an
    /// action. This can be either SHIFT(state), REDUCE(action), or
    /// ERROR.
    fn action(&self, state: Self::StateIndex, token_index: Self::TokenIndex) -> Self::Action;

    /// Returns the action to take if an error occurs in the given
    /// state. This function is the same as the ordinary `action`,
    /// except that it applies not to the user's terminals but to the
    /// "special terminal" `!`.
    fn error_action(&self, state: Self::StateIndex) -> Self::Action;

    /// Action to take if EOF occurs in the given state. This function
    /// is the same as the ordinary `action`, except that it applies
    /// not to the user's terminals but to the "special terminal" `$`.
    fn eof_action(&self, state: Self::StateIndex) -> Self::Action;

    /// If we reduce to a nonterminal in the given state, what state
    /// do we go to? This is infallible due to the nature of LR(1)
    /// grammars.
    fn goto(&self, state: Self::StateIndex, nt: Self::NonterminalIndex) -> Self::StateIndex;

    /// "Upcast" a terminal into a symbol so we can push it onto the
    /// parser stack.
    fn token_to_symbol(&self, token_index: Self::TokenIndex, token: Self::Token) -> Self::Symbol;

    /// Returns the expected tokens in a given state. This is used for
    /// error reporting.
    fn expected_tokens(&self, state: Self::StateIndex) -> Vec<String>;

    /// True if this grammar supports error recovery.
    fn uses_error_recovery(&self) -> bool;

    /// Given error information, creates an error recovery symbol that
    /// we push onto the stack (and supply to user actions).
    fn error_recovery_symbol(&self, recovery: ErrorRecovery<Self>) -> Self::Symbol;

    /// Execute a reduction in the given state: that is, execute user
    /// code. The start location indicates the "starting point" of the
    /// current lookahead that is triggering the reduction (it is
    /// `None` for EOF).
    ///
    /// The `states` and `symbols` vectors represent the internal
    /// state machine vectors; they are given to `reduce` so that it
    /// can pop off states that no longer apply (and consume their
    /// symbols). At the end, it should also push the new state and
    /// symbol produced.
    ///
    /// Returns a `Some` if we reduced the start state and hence
    /// parsing is complete, or if we encountered an irrecoverable
    /// error.
    ///
    /// FIXME. It would be nice to not have so much logic live in
    /// reduce.  It should just be given an iterator of popped symbols
    /// and return the newly produced symbol (or error). We can use
    /// `simulate_reduce` and our own information to drive the rest,
    /// right? This would also allow us -- I think -- to extend error
    /// recovery to cover user-produced errors.
    fn reduce(
        &mut self,
        reduce_index: Self::ReduceIndex,
        start_location: Option<&Self::Location>,
        states: &mut Vec<Self::StateIndex>,
        symbols: &mut Vec<SymbolTriple<Self>>,
    ) -> Option<ParseResult<Self>>;

    /// Returns information about how many states will be popped
    /// during a reduction, and what nonterminal would be produced as
    /// a result.
    fn simulate_reduce(&self, action: Self::ReduceIndex) -> SimulatedReduce<Self>;
}

pub trait ParserAction<D: ParserDefinition>: Copy + Clone + Debug {
    fn as_shift(self) -> Option<D::StateIndex>;
    fn as_reduce(self) -> Option<D::ReduceIndex>;
    fn is_shift(self) -> bool;
    fn is_reduce(self) -> bool;
    fn is_error(self) -> bool;
}

pub enum SimulatedReduce<D: ParserDefinition> {
    Reduce {
        states_to_pop: usize,
        nonterminal_produced: D::NonterminalIndex,
    },

    // This reduce is the "start" fn, so the parse is done.
    Accept,
}

// These aliases are an elaborate hack to get around
// the warnings when you define a type alias like `type Foo<D: Trait>`
#[doc(hidden)]
pub type Location<D> = <D as ParserDefinition>::Location;
#[doc(hidden)]
pub type Token<D> = <D as ParserDefinition>::Token;
#[doc(hidden)]
pub type Error<D> = <D as ParserDefinition>::Error;
#[doc(hidden)]
pub type Success<D> = <D as ParserDefinition>::Success;
#[doc(hidden)]
pub type Symbol<D> = <D as ParserDefinition>::Symbol;

pub type ParseError<D> = crate::ParseError<Location<D>, Token<D>, Error<D>>;
pub type ParseResult<D> = Result<Success<D>, ParseError<D>>;
pub type TokenTriple<D> = (Location<D>, Token<D>, Location<D>);
pub type SymbolTriple<D> = (Location<D>, Symbol<D>, Location<D>);
pub type ErrorRecovery<D> = crate::ErrorRecovery<Location<D>, Token<D>, Error<D>>;

pub struct Parser<D, I>
where
    D: ParserDefinition,
    I: Iterator<Item = Result<TokenTriple<D>, ParseError<D>>>,
{
    definition: D,
    tokens: I,
    states: Vec<D::StateIndex>,
    symbols: Vec<SymbolTriple<D>>,
    last_location: D::Location,
}

enum NextToken<D: ParserDefinition> {
    FoundToken(TokenTriple<D>, D::TokenIndex),
    EOF,
    Done(ParseResult<D>),
}

impl<D, I> Parser<D, I>
where
    D: ParserDefinition,
    I: Iterator<Item = Result<TokenTriple<D>, ParseError<D>>>,
{
    pub fn drive(definition: D, tokens: I) -> ParseResult<D> {
        let last_location = definition.start_location();
        let start_state = definition.start_state();
        Parser {
            definition,
            tokens,
            states: vec![start_state],
            symbols: vec![],
            last_location,
        }
        .parse()
    }

    fn top_state(&self) -> D::StateIndex {
        *self.states.last().unwrap()
    }

    fn parse(&mut self) -> ParseResult<D> {
        // Outer loop: each time we continue around this loop, we
        // shift a new token from the input. We break from the loop
        // when the end of the input is reached (we return early if an
        // error occurs).
        'shift: loop {
            let (mut lookahead, mut token_index) = match self.next_token() {
                NextToken::FoundToken(l, i) => (l, i),
                NextToken::EOF => return self.parse_eof(),
                NextToken::Done(e) => return e,
            };

            debug!("+ SHIFT: {:?}", lookahead);

            debug!("\\ token_index: {:?}", token_index);

            'inner: loop {
                let top_state = self.top_state();
                let action = self.definition.action(top_state, token_index);
                debug!("\\ action: {:?}", action);

                if let Some(target_state) = action.as_shift() {
                    debug!("\\ shift to: {:?}", target_state);

                    // Shift and transition to state `action - 1`
                    let symbol = self.definition.token_to_symbol(token_index, lookahead.1);
                    self.states.push(target_state);
                    self.symbols.push((lookahead.0, symbol, lookahead.2));
                    continue 'shift;
                } else if let Some(reduce_index) = action.as_reduce() {
                    debug!("\\ reduce to: {:?}", reduce_index);

                    if let Some(r) = self.reduce(reduce_index, Some(&lookahead.0)) {
                        return match r {
                            // we reached eof, but still have lookahead
                            Ok(_) => Err(crate::ParseError::ExtraToken { token: lookahead }),
                            Err(e) => Err(e),
                        };
                    }
                } else {
                    debug!("\\ error -- initiating error recovery!");

                    match self.error_recovery(Some(lookahead), Some(token_index)) {
                        NextToken::FoundToken(l, i) => {
                            lookahead = l;
                            token_index = i;
                            continue 'inner;
                        }
                        NextToken::EOF => return self.parse_eof(),
                        NextToken::Done(e) => return e,
                    }
                }
            }
        }
    }

    /// Invoked when we have no more tokens to consume.
    fn parse_eof(&mut self) -> ParseResult<D> {
        loop {
            let top_state = self.top_state();
            let action = self.definition.eof_action(top_state);
            if let Some(reduce_index) = action.as_reduce() {
                if let Some(result) =
                    self.definition
                        .reduce(reduce_index, None, &mut self.states, &mut self.symbols)
                {
                    return result;
                }
            } else {
                match self.error_recovery(None, None) {
                    NextToken::FoundToken(..) => panic!("cannot find token at EOF"),
                    NextToken::Done(e) => return e,
                    NextToken::EOF => continue,
                }
            }
        }
    }

    fn error_recovery(
        &mut self,
        mut opt_lookahead: Option<TokenTriple<D>>,
        mut opt_token_index: Option<D::TokenIndex>,
    ) -> NextToken<D> {
        debug!(
            "\\+ error_recovery(opt_lookahead={:?}, opt_token_index={:?})",
            opt_lookahead, opt_token_index,
        );

        if !self.definition.uses_error_recovery() {
            debug!("\\ error -- no error recovery!");

            return NextToken::Done(Err(
                self.unrecognized_token_error(opt_lookahead, self.top_state())
            ));
        }

        let error = self.unrecognized_token_error(opt_lookahead.clone(), self.top_state());

        let mut dropped_tokens = vec![];

        // We are going to insert ERROR into the lookahead. So, first,
        // perform all reductions from current state triggered by having
        // ERROR in the lookahead.
        loop {
            let state = self.top_state();
            let action = self.definition.error_action(state);
            if let Some(reduce_index) = action.as_reduce() {
                debug!("\\\\ reducing: {:?}", reduce_index);

                if let Some(result) =
                    self.reduce(reduce_index, opt_lookahead.as_ref().map(|l| &l.0))
                {
                    debug!("\\\\ reduced to a result");

                    return NextToken::Done(result);
                }
            } else {
                break;
            }
        }

        // Now try to find the recovery state.
        let states_len = self.states.len();
        let top = 'find_state: loop {
            // Go backwards through the states...
            debug!(
                "\\\\+ error_recovery: find_state loop, {:?} states = {:?}",
                self.states.len(),
                self.states,
            );

            for top in (0..states_len).rev() {
                let state = self.states[top];
                debug!("\\\\\\ top = {:?}, state = {:?}", top, state);

                // ...fetch action for error token...
                let action = self.definition.error_action(state);
                debug!("\\\\\\ action = {:?}", action);
                if let Some(error_state) = action.as_shift() {
                    // If action is a shift that takes us into `error_state`,
                    // and `error_state` can accept this lookahead, we are done.
                    if self.accepts(error_state, &self.states[..=top], opt_token_index) {
                        debug!("\\\\\\ accepted!");
                        break 'find_state top;
                    }
                } else {
                    // ...else, if action is error or reduce, go to next state.
                    continue;
                }
            }

            // Otherwise, if we couldn't find a state that would --
            // after shifting the error token -- accept the lookahead,
            // then drop the lookahead and advance to next token in
            // the input.
            match opt_lookahead.take() {
                // If the lookahead is EOF, we can't drop any more
                // tokens, abort error recovery and just report the
                // original error (it might be nice if we would
                // propagate back the dropped tokens, though).
                None => {
                    debug!("\\\\\\ no more lookahead, report error");
                    return NextToken::Done(Err(error));
                }

                // Else, drop the current token and shift to the
                // next. If there is a next token, we will `continue`
                // to the start of the `'find_state` loop.
                Some(lookahead) => {
                    debug!("\\\\\\ dropping lookahead token");

                    dropped_tokens.push(lookahead);
                    match self.next_token() {
                        NextToken::FoundToken(next_lookahead, next_token_index) => {
                            opt_lookahead = Some(next_lookahead);
                            opt_token_index = Some(next_token_index);
                        }
                        NextToken::EOF => {
                            debug!("\\\\\\ reached EOF");
                            opt_lookahead = None;
                            opt_token_index = None;
                        }
                        NextToken::Done(e) => {
                            debug!("\\\\\\ no more tokens");
                            return NextToken::Done(e);
                        }
                    }
                }
            }
        };

        // If we get here, we are ready to push the error recovery state.

        // We have to compute the span for the error recovery
        // token. We do this first, before we pop any symbols off the
        // stack. There are several possibilities, in order of
        // preference.
        //
        // For the **start** of the message, we prefer to use the start of any
        // popped states. This represents parts of the input we had consumed but
        // had to roll back and ignore.
        //
        // Example:
        //
        //       a + (b + /)
        //              ^ start point is here, since this `+` will be popped off
        //
        // If there are no popped states, but there *are* dropped tokens, we can use
        // the start of those.
        //
        // Example:
        //
        //       a + (b + c e)
        //                  ^ start point would be here
        //
        // Finally, if there are no popped states *nor* dropped tokens, we can use
        // the end of the top-most state.

        let start = if let Some(popped_sym) = self.symbols.get(top) {
            popped_sym.0.clone()
        } else if let Some(dropped_token) = dropped_tokens.first() {
            dropped_token.0.clone()
        } else if top > 0 {
            self.symbols[top - 1].2.clone()
        } else {
            self.definition.start_location()
        };

        // For the end span, here are the possibilities:
        //
        // We prefer to use the end of the last dropped token.
        //
        // Examples:
        //
        //       a + (b + /)
        //              ---
        //       a + (b c)
        //              -
        //
        // But, if there are no dropped tokens, we will use the end of the popped states,
        // if any:
        //
        //       a + /
        //         -
        //
        // If there are neither dropped tokens *or* popped states,
        // then the user is simulating insertion of an operator. In
        // this case, we prefer the start of the lookahead, but
        // fallback to the start if we are at EOF.
        //
        // Examples:
        //
        //       a + (b c)
        //             -

        let end = if let Some(dropped_token) = dropped_tokens.last() {
            dropped_token.2.clone()
        } else if states_len - 1 > top {
            self.symbols.last().unwrap().2.clone()
        } else if let Some(lookahead) = opt_lookahead.as_ref() {
            lookahead.0.clone()
        } else {
            start.clone()
        };

        self.states.truncate(top + 1);
        self.symbols.truncate(top);

        let recover_state = self.states[top];
        let error_action = self.definition.error_action(recover_state);
        let error_state = error_action.as_shift().unwrap();
        self.states.push(error_state);
        let recovery = self.definition.error_recovery_symbol(crate::ErrorRecovery {
            error,
            dropped_tokens,
        });
        self.symbols.push((start, recovery, end));

        match (opt_lookahead, opt_token_index) {
            (Some(l), Some(i)) => NextToken::FoundToken(l, i),
            (None, None) => NextToken::EOF,
            (l, i) => panic!("lookahead and token_index mismatched: {:?}, {:?}", l, i),
        }
    }

    /// The `accepts` function has the job of figuring out whether the
    /// given error state would "accept" the given lookahead. We
    /// basically trace through the LR automaton looking for one of
    /// two outcomes:
    ///
    /// - the lookahead is eventually shifted
    /// - we reduce to the end state successfully (in the case of EOF).
    ///
    /// If we used the pure LR(1) algorithm, we wouldn't need this
    /// function, because we would be guaranteed to error immediately
    /// (and not after some number of reductions). But with an LALR
    /// (or Lane Table) generated automaton, it is possible to reduce
    /// some number of times before encountering an error. Failing to
    /// take this into account can lead error recovery into an
    /// infinite loop (see the `error_recovery_lalr_loop` test) or
    /// produce crappy results (see `error_recovery_lock_in`).
    fn accepts(
        &self,
        error_state: D::StateIndex,
        states: &[D::StateIndex],
        opt_token_index: Option<D::TokenIndex>,
    ) -> bool {
        debug!(
            "\\\\\\+ accepts(error_state={:?}, states={:?}, opt_token_index={:?})",
            error_state, states, opt_token_index,
        );

        let mut states = states.to_vec();
        states.push(error_state);
        loop {
            let mut states_len = states.len();
            let top = states[states_len - 1];
            let action = match opt_token_index {
                None => self.definition.eof_action(top),
                Some(i) => self.definition.action(top, i),
            };

            // If we encounter an error action, we do **not** accept.
            if action.is_error() {
                debug!("\\\\\\\\ accepts: error");
                return false;
            }

            // If we encounter a reduce action, we need to simulate its
            // effect on the state stack.
            if let Some(reduce_action) = action.as_reduce() {
                match self.definition.simulate_reduce(reduce_action) {
                    SimulatedReduce::Reduce {
                        states_to_pop,
                        nonterminal_produced,
                    } => {
                        states_len -= states_to_pop;
                        states.truncate(states_len);
                        let top = states[states_len - 1];
                        let next_state = self.definition.goto(top, nonterminal_produced);
                        states.push(next_state);
                    }

                    SimulatedReduce::Accept => {
                        debug!("\\\\\\\\ accepts: reduce accepts!");
                        return true;
                    }
                }
            } else {
                // If we encounter a shift action, we DO accept.
                debug!("\\\\\\\\ accepts: shift accepts!");
                assert!(action.is_shift());
                return true;
            }
        }
    }

    fn reduce(
        &mut self,
        action: D::ReduceIndex,
        lookahead_start: Option<&D::Location>,
    ) -> Option<ParseResult<D>> {
        self.definition
            .reduce(action, lookahead_start, &mut self.states, &mut self.symbols)
    }

    fn unrecognized_token_error(
        &self,
        token: Option<TokenTriple<D>>,
        top_state: D::StateIndex,
    ) -> ParseError<D> {
        match token {
            Some(token) => crate::ParseError::UnrecognizedToken {
                token,
                expected: self.definition.expected_tokens(top_state),
            },
            None => crate::ParseError::UnrecognizedEOF {
                location: self.last_location.clone(),
                expected: self.definition.expected_tokens(top_state),
            },
        }
    }

    /// Consume the next token from the input and classify it into a
    /// token index. Classification can fail with an error. If there
    /// are no more tokens, signal EOF.
    fn next_token(&mut self) -> NextToken<D> {
        let token = match self.tokens.next() {
            Some(Ok(v)) => v,
            Some(Err(e)) => return NextToken::Done(Err(e)),
            None => return NextToken::EOF,
        };

        self.last_location = token.2.clone();

        let token_index = match self.definition.token_to_index(&token.1) {
            Some(i) => i,
            None => {
                return NextToken::Done(Err(
                    self.unrecognized_token_error(Some(token), self.top_state())
                ))
            }
        };

        NextToken::FoundToken(token, token_index)
    }
}

/// In LALRPOP generated rules, we actually use `i32`, `i16`, or `i8`
/// to represent all of the various indices (we use the smallest one
/// that will fit). So implement `ParserAction` for each of those.
macro_rules! integral_indices {
    ($t:ty) => {
        impl<D: ParserDefinition<StateIndex = $t, ReduceIndex = $t>> ParserAction<D> for $t {
            fn as_shift(self) -> Option<D::StateIndex> {
                if self > 0 {
                    Some(self - 1)
                } else {
                    None
                }
            }

            fn as_reduce(self) -> Option<D::ReduceIndex> {
                if self < 0 {
                    Some(-(self + 1))
                } else {
                    None
                }
            }

            fn is_shift(self) -> bool {
                self > 0
            }

            fn is_reduce(self) -> bool {
                self < 0
            }

            fn is_error(self) -> bool {
                self == 0
            }
        }
    };
}

integral_indices!(i32);
integral_indices!(i16);
integral_indices!(i8);
