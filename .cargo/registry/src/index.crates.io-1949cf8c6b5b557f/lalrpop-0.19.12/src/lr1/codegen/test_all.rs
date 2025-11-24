//! A compiler from an LR(1) table to a [recursive ascent] parser.
//!
//! [recursive ascent]: https://en.wikipedia.org/wiki/Recursive_ascent_parser

use crate::grammar::repr::{Grammar, NonterminalString, TypeParameter};
use crate::lr1::core::*;
use crate::rust::RustWrite;
use std::io::{self, Write};
use crate::util::Sep;

use super::base::CodeGenerator;

pub fn compile<'grammar, W: Write>(
    grammar: &'grammar Grammar,
    user_start_symbol: NonterminalString,
    start_symbol: NonterminalString,
    states: &[LR1State<'grammar>],
    out: &mut RustWrite<W>,
) -> io::Result<()> {
    let mut ascent =
        CodeGenerator::new_test_all(grammar, user_start_symbol, start_symbol, states, out);
    ascent.write()
}

struct TestAll;

impl<'ascent, 'grammar, W: Write> CodeGenerator<'ascent, 'grammar, W, TestAll> {
    fn new_test_all(
        grammar: &'grammar Grammar,
        user_start_symbol: NonterminalString,
        start_symbol: NonterminalString,
        states: &'ascent [LR1State<'grammar>],
        out: &'ascent mut RustWrite<W>,
    ) -> Self {
        CodeGenerator::new(
            grammar,
            user_start_symbol,
            start_symbol,
            states,
            out,
            true,
            "super",
            TestAll,
        )
    }

    fn write(&mut self) -> io::Result<()> {
        self.write_parse_mod(|this| {
            this.write_parser_fn()?;

            rust!(this.out, "#[cfg_attr(rustfmt, rustfmt_skip)]");
            rust!(this.out, "mod {}ascent {{", this.prefix);
            super::ascent::compile(
                this.grammar,
                this.user_start_symbol.clone(),
                this.start_symbol.clone(),
                this.states,
                "super::super::super",
                this.out,
            )?;
            let pub_use = format!(
                "{}use self::{}parse{}::{}Parser;",
                this.grammar.nonterminals[&this.user_start_symbol].visibility,
                this.prefix,
                this.start_symbol,
                this.user_start_symbol
            );
            rust!(this.out, "{}", pub_use);
            rust!(this.out, "}}");

            rust!(this.out, "#[cfg_attr(rustfmt, rustfmt_skip)]");
            rust!(this.out, "mod {}parse_table {{", this.prefix);
            super::parse_table::compile(
                this.grammar,
                this.user_start_symbol.clone(),
                this.start_symbol.clone(),
                this.states,
                "super::super::super",
                this.out,
            )?;
            rust!(this.out, "{}", pub_use);
            rust!(this.out, "}}");

            Ok(())
        })
    }

    fn write_parser_fn(&mut self) -> io::Result<()> {
        self.start_parser_fn()?;

        if self.grammar.intern_token.is_some() {
            rust!(self.out, "let _ = self.builder;");
        }
        // parse input using both methods:
        self.call_delegate("ascent")?;
        self.call_delegate("parse_table")?;

        // check that result is the same either way:
        rust!(
            self.out,
            "assert_eq!({}ascent, {}parse_table);",
            self.prefix,
            self.prefix
        );

        rust!(self.out, "return {}ascent;", self.prefix);

        self.end_parser_fn()?;

        Ok(())
    }

    fn call_delegate(&mut self, delegate: &str) -> io::Result<()> {
        let non_lifetimes: Vec<_> = self
            .grammar
            .type_parameters
            .iter()
            .filter(|&tp| match *tp {
                TypeParameter::Lifetime(_) => false,
                TypeParameter::Id(_) => true,
            })
            .cloned()
            .collect();
        let parameters = if non_lifetimes.is_empty() {
            String::new()
        } else if self.grammar.intern_token.is_some() {
            format!("::<{}>", Sep(", ", &non_lifetimes))
        } else {
            format!("::<{}, _, _>", Sep(", ", &non_lifetimes))
        };
        rust!(
            self.out,
            "let {}{} = {}{}::{}Parser::new().parse{}(",
            self.prefix,
            delegate,
            self.prefix,
            delegate,
            self.user_start_symbol,
            parameters
        );
        for parameter in &self.grammar.parameters {
            rust!(self.out, "{},", parameter.name);
        }
        if self.grammar.intern_token.is_none() {
            rust!(self.out, "{}tokens0.clone(),", self.prefix);
        }
        rust!(self.out, ");");
        Ok(())
    }
}
