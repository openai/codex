//! A compiler from an LR(1) table to a [recursive ascent] parser.
//!
//! [recursive ascent]: https://en.wikipedia.org/wiki/Recursive_ascent_parser

use crate::collections::Multimap;
use crate::grammar::repr::{
    Grammar, NonterminalString, Production, Symbol, TerminalString, TypeParameter, TypeRepr,
    Visibility, WhereClause,
};
use crate::lr1::core::*;
use crate::lr1::lookahead::Token;
use crate::lr1::state_graph::StateGraph;
use crate::rust::RustWrite;
use crate::tls::Tls;
use crate::util::{Escape, Sep};
use std::io::{self, Write};

use super::base::CodeGenerator;

pub fn compile<'grammar, W: Write>(
    grammar: &'grammar Grammar,
    user_start_symbol: NonterminalString,
    start_symbol: NonterminalString,
    states: &[LR1State<'grammar>],
    action_module: &str,
    out: &mut RustWrite<W>,
) -> io::Result<()> {
    let graph = StateGraph::new(states);
    let mut ascent = CodeGenerator::new_ascent(
        grammar,
        user_start_symbol,
        start_symbol,
        &graph,
        states,
        action_module,
        out,
    );
    ascent.write()
}

struct RecursiveAscent<'ascent, 'grammar> {
    graph: &'ascent StateGraph,

    /// for each state, the set of symbols that it will require for
    /// input
    state_inputs: Vec<StackSuffix<'grammar>>,

    /// type parameters for the `Nonterminal` type
    nonterminal_type_params: Vec<TypeParameter>,

    nonterminal_where_clauses: Vec<WhereClause>,
}

/// Tracks the suffix of the stack (that is, top-most elements) that any
/// particular state is aware of. We break the suffix into two parts:
/// optional and fixed, which always look like this:
///
/// ```
/// ... A B C X Y Z
/// ~~~ ~~~~~ ~~~~~
///  |    |     |
///  |    |   Fixed (top of the stack)
///  |    |
///  |  Optional (will be popped after the fixed portion)
///  |
/// Prefix (stuff we don't know about that is also on the stack
/// ```
///
/// The idea of an "optional" member is not that it may or may not be
/// on the stack. The entire suffix will always be on the stack. An
/// *optional* member is one that *we* may or may not *consume*. So
/// the above stack suffix could occur given a state with items like:
///
/// ```
/// NT1 = A B C X Y Z (*) "."
/// NT2 = X Y Z (*) ","
/// ```
///
/// Depending on what comes next, if we reduce NT1, we will consume
/// all six symbols, but if we reduce NT2, we will only reduce three.
#[derive(Copy, Clone, Debug)]
struct StackSuffix<'grammar> {
    /// all symbols that are known to be on the stack (optional + fixed).
    all: &'grammar [Symbol],

    /// optional symbols will be consumed by *some* reductions in this
    /// state, but not all
    len_optional: usize,
}

impl<'grammar> StackSuffix<'grammar> {
    fn len(&self) -> usize {
        self.all.len()
    }

    /// returns the (optional, fixed) -- number of optional
    /// items in stack prefix and number of fixed
    fn optional_fixed_lens(&self) -> (usize, usize) {
        (self.len_optional, self.len() - self.len_optional)
    }

    fn is_not_empty(&self) -> bool {
        self.len() > 0
    }

    fn optional(&self) -> &'grammar [Symbol] {
        &self.all[..self.len_optional]
    }

    fn fixed(&self) -> &'grammar [Symbol] {
        &self.all[self.len_optional..]
    }
}

impl<'ascent, 'grammar, W: Write>
    CodeGenerator<'ascent, 'grammar, W, RecursiveAscent<'ascent, 'grammar>>
{
    fn new_ascent(
        grammar: &'grammar Grammar,
        user_start_symbol: NonterminalString,
        start_symbol: NonterminalString,
        graph: &'ascent StateGraph,
        states: &'ascent [LR1State<'grammar>],
        action_module: &str,
        out: &'ascent mut RustWrite<W>,
    ) -> Self {
        let (nonterminal_type_params, nonterminal_where_clauses) =
            Self::filter_type_parameters_and_where_clauses(
                grammar,
                grammar.types.nonterminal_types(),
            );

        let state_inputs = states
            .iter()
            .map(|state| Self::state_input_for(state))
            .collect();

        CodeGenerator::new(
            grammar,
            user_start_symbol,
            start_symbol,
            states,
            out,
            false,
            action_module,
            RecursiveAscent {
                graph,
                state_inputs,
                nonterminal_type_params,
                nonterminal_where_clauses,
            },
        )
    }

    /// Compute the stack suffix that the state expects on entry.
    fn state_input_for(state: &'ascent LR1State<'grammar>) -> StackSuffix<'grammar> {
        let max_prefix = state.max_prefix();
        let will_pop = state.will_pop();
        StackSuffix {
            all: max_prefix,
            len_optional: max_prefix.len() - will_pop.len(),
        }
    }

    fn write(&mut self) -> io::Result<()> {
        self.write_parse_mod(|this| {
            this.write_start_fn()?;
            rust!(this.out, "");
            this.write_return_type_defn()?;
            for i in 0..this.states.len() {
                this.write_state_fn(StateIndex(i))?;
            }
            Ok(())
        })
    }

    fn write_return_type_defn(&mut self) -> io::Result<()> {
        // sometimes some of the variants are not used, particularly
        // if we are generating multiple parsers from the same file:
        rust!(self.out, "#[allow(dead_code)]");
        rust!(
            self.out,
            "pub(crate) enum {}Nonterminal<{}>",
            self.prefix,
            Sep(", ", &self.custom.nonterminal_type_params)
        );

        if !self.custom.nonterminal_where_clauses.is_empty() {
            rust!(
                self.out,
                " where {}",
                Sep(", ", &self.custom.nonterminal_where_clauses)
            );
        }

        rust!(self.out, " {{");

        // make an enum with one variant per nonterminal; I considered
        // making different enums per state, but this would mean we
        // have to unwrap and rewrap as we pass up the stack, which
        // seems silly
        for nt in self.grammar.nonterminals.keys() {
            let ty = self
                .types
                .spanned_type(self.types.nonterminal_type(nt).clone());
            rust!(self.out, "{}({}),", Escape(nt), ty);
        }

        rust!(self.out, "}}");
        Ok(())
    }

    // Generates a function `parse_Foo` that will parse an entire
    // input as `Foo`. An error is reported if the entire input is not
    // consumed.
    fn write_start_fn(&mut self) -> io::Result<()> {
        let phantom_data = self.phantom_data_expr();
        self.start_parser_fn()?;
        self.define_tokens()?;

        self.next_token("lookahead", "tokens")?;
        rust!(
            self.out,
            "match {}state0({}&mut {}tokens, {}lookahead, {})? {{",
            self.prefix,
            self.grammar.user_parameter_refs(),
            self.prefix,
            self.prefix,
            phantom_data
        );

        // extra tokens?
        rust!(self.out, "(Some({}lookahead), _) => {{", self.prefix);
        rust!(
            self.out,
            "Err({}lalrpop_util::ParseError::ExtraToken {{ token: {}lookahead }})",
            self.prefix,
            self.prefix
        );
        rust!(self.out, "}}");

        // otherwise, we expect to see only the goal terminal
        rust!(
            self.out,
            "(None, {}Nonterminal::{}((_, {}nt, _))) => {{",
            self.prefix,
            Escape(&self.start_symbol),
            self.prefix
        );
        rust!(self.out, "Ok({}nt)", self.prefix);
        rust!(self.out, "}}");

        // nothing else should be possible
        rust!(self.out, "_ => unreachable!(),");
        rust!(self.out, "}}");

        self.end_parser_fn()
    }

    /// Writes the function that corresponds to a given state. This
    /// function takes arguments corresponding to the stack slots of
    /// the LR(1) machine. It consumes tokens and handles reduces
    /// etc. It will return once it has popped at least one symbol off
    /// of the LR stack.
    ///
    /// Note that for states which have a custom kind, this function
    /// emits nothing at all other than a possible comment explaining
    /// the state.
    fn write_state_fn(&mut self, this_index: StateIndex) -> io::Result<()> {
        let this_state = &self.states[this_index.0];
        let inputs = self.custom.state_inputs[this_index.0];

        rust!(self.out, "");

        // Leave a comment explaining what this state is.
        if Tls::session().emit_comments {
            rust!(self.out, "// State {}", this_index.0);
            rust!(self.out, "//     AllInputs = {:?}", inputs.all);
            rust!(self.out, "//     OptionalInputs = {:?}", inputs.optional());
            rust!(self.out, "//     FixedInputs = {:?}", inputs.fixed());
            rust!(
                self.out,
                "//     WillPushLen = {:?}",
                this_state.will_push().len()
            );
            rust!(self.out, "//     WillPush = {:?}", this_state.will_push());
            rust!(
                self.out,
                "//     WillProduce = {:?}",
                this_state.will_produce()
            );
            rust!(self.out, "//");
            for item in this_state.items.vec.iter() {
                rust!(self.out, "//     {:?}", item);
            }
            rust!(self.out, "//");
            for (terminal, action) in &this_state.shifts {
                rust!(self.out, "//   {:?} -> {:?}", terminal, action);
            }
            for &(ref tokens, action) in &this_state.reductions {
                rust!(self.out, "//   {:?} -> {:?}", tokens, action);
            }
            rust!(self.out, "//");
            for (nt, state) in &this_state.gotos {
                rust!(self.out, "//     {:?} -> {:?}", nt, state);
            }
        }

        self.emit_state_fn_header("state", this_index.0, inputs)?;

        // possibly move some fixed inputs into optional stack slots
        let stack_suffix = self.adjust_inputs(this_index, inputs)?;

        // set to true if goto actions are worth generating
        let mut fallthrough = false;

        rust!(self.out, "match {}lookahead {{", self.prefix);

        // first emit shifts:
        for (terminal, &next_index) in &this_state.shifts {
            let sym_name = format!("{}sym{}", self.prefix, inputs.len());
            self.consume_terminal(terminal, sym_name)?;

            // transition to the new state
            if self.transition("result", stack_suffix, next_index, &["tokens"])? {
                fallthrough = true;
            }

            rust!(self.out, "}}");
        }

        // now emit reduces. It frequently happens that many tokens
        // trigger the same reduction, so group these by the
        // production that we are going to be reducing.
        let reductions: Multimap<_, Vec<_>> = this_state
            .reductions
            .iter()
            .flat_map(|&(ref tokens, production)| tokens.iter().map(move |t| (production, t)))
            .collect();
        for (production, tokens) in reductions {
            for (index, token) in tokens.iter().enumerate() {
                let pattern = match *token {
                    Token::Terminal(ref s) => format!("Some({})", self.match_terminal_pattern(s)),
                    Token::Error => {
                        panic!("Error recovery is not implemented for recursive ascent parsers")
                    }
                    Token::EOF => "None".to_string(),
                };
                if index < tokens.len() - 1 {
                    rust!(self.out, "{} |", pattern);
                } else {
                    rust!(self.out, "{} => {{", pattern);
                }
            }

            self.emit_reduce_action("result", stack_suffix, production)?;

            if !production.symbols.is_empty() {
                // if we popped anything off of the stack, then this frame is done
                rust!(self.out, "return Ok({}result);", self.prefix);
            } else {
                fallthrough = true;
            }

            rust!(self.out, "}}");
        }

        // if we hit this, the next token is not recognized, so generate an error
        rust!(self.out, "_ => {{");
        // The terminals which would have resulted in a successful parse in this state
        let successful_terminals = self.grammar.terminals.all.iter().filter(|&terminal| {
            this_state.shifts.contains_key(terminal)
                || this_state
                    .reductions
                    .iter()
                    .any(|&(ref t, _)| t.contains(&Token::Terminal(terminal.clone())))
        });

        rust!(self.out, "let {}expected = alloc::vec![", self.prefix);
        for terminal in successful_terminals {
            rust!(self.out, "r###\"{}\"###.to_string(),", terminal);
        }
        rust!(self.out, "];");

        // check if we've found an unrecognized token or EOF
        rust!(self.out, "return Err(");
        rust!(self.out, "match {}lookahead {{", self.prefix);

        rust!(self.out, "Some({}token) => {{", self.prefix);
        rust!(
            self.out,
            "{}lalrpop_util::ParseError::UnrecognizedToken {{",
            self.prefix
        );
        rust!(self.out, "token: {}token,", self.prefix);
        rust!(self.out, "expected: {}expected,", self.prefix);
        rust!(self.out, "}}");
        rust!(self.out, "}}");

        rust!(self.out, "None => {{");

        // find the location of the last symbol on stack
        let (optional, fixed) = stack_suffix.optional_fixed_lens();
        if fixed > 0 {
            rust!(
                self.out,
                "let {}location = {}sym{}.2.clone();",
                self.prefix,
                self.prefix,
                stack_suffix.len() - 1
            );
        } else if optional > 0 {
            rust!(self.out, "let {}location = ", self.prefix);
            for index in (0..optional).rev() {
                rust!(
                    self.out,
                    "{}sym{}.as_ref().map(|sym| sym.2.clone()).unwrap_or_else(|| {{",
                    self.prefix,
                    index
                );
            }
            rust!(self.out, "Default::default()");
            for _ in 0..optional {
                rust!(self.out, "}})");
            }
            rust!(self.out, ";");
        } else {
            rust!(
                self.out,
                "let {}location = Default::default();",
                self.prefix
            );
        }

        rust!(
            self.out,
            "{}lalrpop_util::ParseError::UnrecognizedEOF {{",
            self.prefix
        );
        rust!(self.out, "location: {}location,", self.prefix);
        rust!(self.out, "expected: {}expected,", self.prefix);
        rust!(self.out, "}}");
        rust!(self.out, "}}");

        rust!(self.out, "}}"); // Error match
        rust!(self.out, ")");

        rust!(self.out, "}}"); // Wildcard match case
        rust!(self.out, "}}"); // match

        // finally, emit gotos (if relevant)
        if fallthrough && !this_state.gotos.is_empty() {
            rust!(self.out, "loop {{");

            // In most states, we know precisely when the top stack
            // slot will be consumed (basically, when we reduce or
            // when we transition to another state). But in some states,
            // we may not know. Consider:
            //
            //     X = A (*) "0" ["."]
            //     X = A (*) B ["."]
            //     B = (*) "0" "1" ["."]
            //
            // Now if we see a `"0"` this *could* be the start of a `B
            // = "0" "1"` or it could be the continuation of `X = A
            // "0"`. We won't know until we see the *next* character
            // (which will either be `"0"` or `"."`). If it turns out to be
            // `X = A "0"`, then the state handling the `"0"` will reduce
            // and consume the `A` and the `"0"`. But otherwise it will shift
            // the `"1"` and leave the `A` unprocessed.
            //
            // In cases like this, the `adjust_inputs` routine will
            // have taken the top of the stack ("A") and put it into
            // an `Option`. After the state processing the `"0"`
            // returns then, we can check this option to see whether
            // it has popped the `"A"` (in which case we ought to
            // return) or not (in which case we ought to shift the `B`
            // value that it returned to us).
            let top_slot_optional =
                { stack_suffix.is_not_empty() && stack_suffix.fixed().is_empty() };
            if top_slot_optional {
                rust!(
                    self.out,
                    "if {}sym{}.is_none() {{",
                    self.prefix,
                    stack_suffix.len() - 1
                );
                rust!(self.out, "return Ok({}result);", self.prefix);
                rust!(self.out, "}}");
            }

            rust!(
                self.out,
                "let ({}lookahead, {}nt) = {}result;",
                self.prefix,
                self.prefix,
                self.prefix
            );

            rust!(self.out, "match {}nt {{", self.prefix);
            for (ref nt, &next_index) in &this_state.gotos {
                // The nonterminal we are shifting becomes symN, where
                // N is the number of inputs to this state (which are
                // numbered sym0..sym(N-1)). It is never optional
                // because we always transition to a state with at
                // least *one* fixed input.
                rust!(
                    self.out,
                    "{}Nonterminal::{}({}sym{}) => {{",
                    self.prefix,
                    Escape(nt),
                    self.prefix,
                    stack_suffix.len()
                );
                self.transition("result", stack_suffix, next_index, &["tokens", "lookahead"])?;
                rust!(self.out, "}}");
            }

            // Errors are not possible in the goto phase; a missing entry
            // indicates parse successfully completed, so just bail out.
            if this_state.gotos.len() != self.grammar.nonterminals.keys().len() {
                rust!(self.out, "_ => {{");
                rust!(
                    self.out,
                    "return Ok(({}lookahead, {}nt));",
                    self.prefix,
                    self.prefix
                );
                rust!(self.out, "}}");
            }

            rust!(self.out, "}}"); // match

            rust!(self.out, "}}"); // while/loop
        } else if fallthrough {
            rust!(self.out, "return Ok({}result);", self.prefix);
        }

        rust!(self.out, "}}"); // fn

        Ok(())
    }

    fn emit_state_fn_header(
        &mut self,
        fn_kind: &str,   // e.g. "state", "custom"
        fn_index: usize, // state index, custom kind index, etc
        suffix: StackSuffix<'grammar>,
    ) -> io::Result<()> {
        let optional_prefix = suffix.optional();
        let fixed_prefix = suffix.fixed();

        let triple_type = self.triple_type();
        let parse_error_type = self.types.parse_error_type();

        let (fn_args, starts_with_terminal) = self.fn_args(optional_prefix, fixed_prefix);

        self.out
            .fn_header(
                &Visibility::Priv,
                format!("{}{}{}", self.prefix, fn_kind, fn_index),
            )
            .with_grammar(self.grammar)
            .with_type_parameters(Some(format!(
                "{}TOKENS: Iterator<Item=Result<{},{}>>",
                self.prefix, triple_type, parse_error_type
            )))
            .with_parameters(fn_args)
            .with_return_type(format!(
                "core::result::Result<(core::option::Option<{}>, {}Nonterminal<{}>), {}>",
                triple_type,
                self.prefix,
                Sep(", ", &self.custom.nonterminal_type_params),
                parse_error_type,
            ))
            .emit()?;

        rust!(self.out, "{{");

        rust!(
            self.out,
            "let mut {}result: (core::option::Option<{}>, {}Nonterminal<{}>);",
            self.prefix,
            triple_type,
            self.prefix,
            Sep(", ", &self.custom.nonterminal_type_params),
        );

        // shift lookahead is necessary; see `starts_with_terminal` above
        if starts_with_terminal {
            self.next_token("lookahead", "tokens")?;
        }

        Ok(())
    }

    // Compute the set of arguments that the function for a state or
    // custom-kind expects.  The argument `symbols` represents the top
    // portion of the stack which this function expects to be given.
    // Each of them will be given an argument like `sym3: &mut
    // Option<Sym3>` where `Sym3` is the type of the symbol.
    //
    // Returns a list of argument names and a flag if this fn resulted
    // from pushing a terminal (in which case the lookahead must be
    // computed internally).
    fn fn_args(
        &mut self,
        optional_prefix: &[Symbol],
        fixed_prefix: &[Symbol],
    ) -> (Vec<String>, bool) {
        assert!(
            // start state:
            (optional_prefix.is_empty() && fixed_prefix.is_empty()) ||
            /* any other state: */ !fixed_prefix.is_empty()
        );
        let triple_type = self.triple_type();

        // to reduce the size of the generated code, if the state
        // results from shifting a terminal, then we do not pass the
        // lookahead in as an argument, but rather we load it as the
        // first thing in this function; this saves some space because
        // there are more edges than there are states in the graph.
        let starts_with_terminal = fixed_prefix
            .last()
            .map(Symbol::is_terminal)
            .unwrap_or(false);

        let mut base_args = vec![format!("{}tokens: &mut {}TOKENS", self.prefix, self.prefix)];
        if !starts_with_terminal {
            base_args.push(format!(
                "{}lookahead: core::option::Option<{}>",
                self.prefix, triple_type,
            ));
        }

        // "Optional symbols" may or may not be consumed, so take an
        // `&mut Option`
        let optional_args = (0..optional_prefix.len()).map(|i| {
            format!(
                "{}sym{}: &mut core::option::Option<{}>",
                self.prefix,
                i,
                self.types
                    .spanned_type(optional_prefix[i].ty(self.types).clone()),
            )
        });

        // "Fixed symbols" will be consumed before we return, so take the value itself
        let fixed_args = (0..fixed_prefix.len()).map(|i| {
            format!(
                "{}sym{}: {}",
                self.prefix,
                optional_prefix.len() + i,
                self.types
                    .spanned_type(fixed_prefix[i].ty(self.types).clone())
            )
        });

        let all_args = base_args
            .into_iter()
            .chain(optional_args)
            .chain(fixed_args)
            .chain(Some(format!("_: {}", self.phantom_data_type())))
            .collect();

        (all_args, starts_with_terminal)
    }

    /// Examine the states that we may transition to. Unless this is
    /// the start state, we will always take at least 1 fixed input:
    /// the most recently pushed symbol (let's call it `symX`), and we
    /// may have others as well. But if this state can transition to
    /// another state can takes some of those inputs as optional
    /// parameters, we need to convert them them options. This
    /// function thus emits code to move each sum `symX` into an
    /// option, and returns an adjusted stack-suffix that reflects the
    /// changes made.
    fn adjust_inputs(
        &mut self,
        state_index: StateIndex,
        inputs: StackSuffix<'grammar>,
    ) -> io::Result<StackSuffix<'grammar>> {
        let mut result = inputs;

        let top_opt = self
            .custom
            .graph
            .successors(state_index)
            .iter()
            .any(|succ_state| {
                let succ_inputs = &self.custom.state_inputs[succ_state.0];

                // Check for a successor state with a suffix like:
                //
                //     ... OPT_1 ... OPT_N FIXED_1
                //
                // (Remember that *every* successor state will have
                // at least one fixed input.)
                //
                // So basically we are looking for states
                // that, when they return, may *optionally* have consumed
                // the top of our stack.
                assert!(!succ_inputs.fixed().is_empty());
                succ_inputs.fixed().len() == 1 && !succ_inputs.optional().is_empty()
            });

        // If we find a successor that may optionally consume the top
        // of our stack, convert our fixed inputs into optional ones.
        //
        // (Here we convert *all* fixed inputs. Honestly, I can't
        // remember if this is necessary, or just for simplicity. I
        // suspect the latter. --nmatsakis)
        if top_opt {
            let start_num = inputs.optional().len();
            for sym_num in (start_num..start_num + inputs.fixed().len()) {
                rust!(
                    self.out,
                    "let {}sym{} = &mut Some({}sym{});",
                    self.prefix,
                    sym_num,
                    self.prefix,
                    sym_num
                );
            }
            result.len_optional = result.len();
        }

        Ok(result)
    }

    /// Given that we have, locally, `optional` number of optional stack slots
    /// followed by `fixed` number of fixed stack slots, prepare the inputs
    /// to be supplied to `inputs`. Returns a string of names for this inputs.
    fn pop_syms(
        &mut self,
        optional: usize,
        fixed: usize,
        inputs: StackSuffix<'grammar>,
    ) -> io::Result<Vec<String>> {
        let total_have = optional + fixed;
        let total_need = inputs.len();
        (total_have - total_need..total_have) // number relative to us
            .zip(0..total_need) // number relative to them
            .map(|(h, n)| {
                let name = format!("{}sym{}", self.prefix, h);
                let have_optional = h < optional;
                let need_optional = n < inputs.len_optional;

                // if we have something stored in an `Option`, but the next state
                // consumes it unconditionally, then "pop" it
                if have_optional && !need_optional {
                    rust!(self.out, "let {} = {}.take().unwrap();", name, name);
                } else {
                    // we should never have something stored
                    // unconditionally that the next state only
                    // "maybe" consumes -- we should have fixed this
                    // in the `adjust_inputs` phase
                    assert_eq!(have_optional, need_optional);
                }
                Ok(name)
            })
            .collect()
    }

    /// Emit code to shift/goto into the state `next_index`. Returns
    /// `true` if the current state may be valid after the target
    /// state returns, or `false` if `transition` will just return
    /// afterwards.
    ///
    /// # Arguments
    ///
    /// - `into_result`: name of variable to store result from target state into
    /// - `stack_suffix`: the suffix of the LR stack that current state is aware of,
    ///   and how it is distributed into optional/fixed slots
    /// - `next_index`: target state
    /// - `other_args`: other arguments we are threading along
    fn transition(
        &mut self,
        into_result: &str,
        stack_suffix: StackSuffix<'grammar>,
        next_index: StateIndex,
        other_args: &[&str],
    ) -> io::Result<bool> {
        // the depth of the suffix of the stack that we are aware of
        // in the current state, including the newly shifted token
        let (optional, mut fixed) = stack_suffix.optional_fixed_lens();
        fixed += 1; // we just shifted another symbol
        let total = optional + fixed;
        assert!(total == stack_suffix.len() + 1);

        // symbols that the next state expects; will always be include
        // at least one fixed input
        let next_inputs = self.custom.state_inputs[next_index.0];
        assert!(!next_inputs.fixed().is_empty());
        assert!(next_inputs.len() <= total);

        let transfer_syms = self.pop_syms(optional, fixed, next_inputs)?;

        let other_args = other_args
            .iter()
            .map(|s| format!("{}{}", self.prefix, s))
            .collect();

        let fn_name = format!("{}state{}", self.prefix, next_index.0);

        // invoke next state, transferring the top `m` tokens
        let phantom_data_expr = self.phantom_data_expr();
        rust!(
            self.out,
            "{}{} = {}({}{}, {}, {})?;",
            self.prefix,
            into_result,
            fn_name,
            self.grammar.user_parameter_refs(),
            Sep(", ", &other_args),
            Sep(", ", &transfer_syms),
            phantom_data_expr
        );

        // if the target state takes at least **two** fixed tokens,
        // then it will have consumed the top of **our** stack frame,
        // so we should just return
        if next_inputs.fixed().len() >= 2 {
            rust!(self.out, "return Ok({}{});", self.prefix, into_result);
            Ok(false)
        } else {
            Ok(true)
        }
    }

    /// Executes a reduction of `production`, storing the result into
    /// the variable `into_var`, which should have type
    /// `(Option<(L,T,L)>, Nonterminal)`.
    fn emit_reduce_action(
        &mut self,
        into_var: &str,
        stack_suffix: StackSuffix<'grammar>,
        production: &'grammar Production,
    ) -> io::Result<()> {
        let loc_type = self.types.terminal_loc_type();

        let (optional, fixed) = stack_suffix.optional_fixed_lens();
        let production_inputs = StackSuffix {
            all: &production.symbols,
            len_optional: 0,
        };
        let transfer_syms = self.pop_syms(optional, fixed, production_inputs)?;

        // identify the "start" and "end" location for this production; this
        // is typically the start of the first symbol and end of the last symbol we are
        // reducing; but in the case of an empty production, it will come from the
        // lookahead or the end of the last symbol pushed
        if let (Some(first_sym), Some(last_sym)) = (transfer_syms.first(), transfer_syms.last()) {
            rust!(
                self.out,
                "let {}start = {}.0.clone();",
                self.prefix,
                first_sym
            );
            rust!(self.out, "let {}end = {}.2.clone();", self.prefix, last_sym);
        } else if stack_suffix.len() > 0 {
            // we pop no symbols, so grab from the top of the stack
            // (unless we are in the start state)
            let top = stack_suffix.len() - 1;
            if !stack_suffix.fixed().is_empty() {
                rust!(
                    self.out,
                    "let {p}start = {p}lookahead.as_ref().map(|o| o.0.clone()).unwrap_or_else(|| {p}sym{top}.2.clone());",
                    p = self.prefix,
                    top = top
                );
            } else {
                // top of stack is optional; should not have been popped yet tho
                rust!(
                    self.out,
                    "let {p}start = {p}lookahead.as_ref().map(|o| o.0.clone()).unwrap_or_else(|| {p}sym{top}.as_ref().unwrap().2.clone());",
                    p = self.prefix,
                    top = top
                );
            }
            rust!(self.out, "let {p}end = {p}start;", p = self.prefix);
        } else {
            // this only occurs in the start state
            rust!(
                self.out,
                "let {}start: {} = core::default::Default::default();",
                self.prefix,
                loc_type,
            );
            rust!(self.out, "let {p}end = {p}start;", p = self.prefix);
        }

        let transferred_syms = transfer_syms.len();

        let mut args = transfer_syms;
        if transferred_syms == 0 {
            args.push(format!("&{}start", self.prefix));
            args.push(format!("&{}end", self.prefix));
        }

        // invoke the action code
        let is_fallible = self.grammar.action_is_fallible(production.action);
        if is_fallible {
            rust!(
                self.out,
                "let {}nt = {}::{}action{}::<{}>({}{})?;",
                self.prefix,
                self.action_module,
                self.prefix,
                production.action.index(),
                Sep(", ", &self.grammar.non_lifetime_type_parameters()),
                self.grammar.user_parameter_refs(),
                Sep(", ", &args)
            )
        } else {
            rust!(
                self.out,
                "let {}nt = {}::{}action{}::<{}>({}{});",
                self.prefix,
                self.action_module,
                self.prefix,
                production.action.index(),
                Sep(", ", &self.grammar.non_lifetime_type_parameters()),
                self.grammar.user_parameter_refs(),
                Sep(", ", &args)
            )
        }

        // wrap up the produced value into `Nonterminal` along with
        rust!(
            self.out,
            "let {}nt = {}Nonterminal::{}((",
            self.prefix,
            self.prefix,
            Escape(&production.nonterminal)
        );
        rust!(self.out, "{}start,", self.prefix);
        rust!(self.out, "{}nt,", self.prefix);
        rust!(self.out, "{}end,", self.prefix);
        rust!(self.out, "));");

        // wrap up the result along with the (unused) lookahead
        rust!(
            self.out,
            "{}{} = ({}lookahead, {}nt);",
            self.prefix,
            into_var,
            self.prefix,
            self.prefix
        );

        Ok(())
    }

    /// Emit a pattern that matches `id` but doesn't extract any data.
    fn match_terminal_pattern(&mut self, id: &TerminalString) -> String {
        let pattern = self.grammar.pattern(id).map(&mut |_| "_");
        let pattern = format!("{}", pattern);
        format!("(_, {}, _)", pattern)
    }

    /// Emit a pattern that matches `id` and extracts its value, storing
    /// that value as `let_name`.
    fn consume_terminal(&mut self, id: &TerminalString, let_name: String) -> io::Result<()> {
        let mut pattern_names = vec![];
        let pattern = self.grammar.pattern(id).map(&mut |_| {
            let index = pattern_names.len();
            pattern_names.push(format!("{}tok{}", self.prefix, index));
            pattern_names.last().cloned().unwrap()
        });

        let mut pattern = format!("{}", pattern);
        if pattern_names.is_empty() {
            pattern_names.push(format!("{}tok", self.prefix));
            pattern = format!("{}tok @ {}", self.prefix, pattern);
        }

        pattern = format!("({}loc1, {}, {}loc2)", self.prefix, pattern, self.prefix);

        rust!(self.out, "Some({}) => {{", pattern);

        rust!(
            self.out,
            "let {} = ({}loc1, ({}), {}loc2);",
            let_name,
            self.prefix,
            pattern_names.join(", "),
            self.prefix
        );

        Ok(())
    }

    fn triple_type(&self) -> TypeRepr {
        self.types.triple_type()
    }

    fn next_token(&mut self, lookahead: &str, tokens: &str) -> io::Result<()> {
        rust!(
            self.out,
            "let {}{} = match {}{}.next() {{",
            self.prefix,
            lookahead,
            self.prefix,
            tokens
        );
        rust!(self.out, "Some(Ok(v)) => Some(v),");
        rust!(self.out, "Some(Err(e)) => return Err(e),");
        rust!(self.out, "None => None,");
        rust!(self.out, "}};");
        Ok(())
    }
}
