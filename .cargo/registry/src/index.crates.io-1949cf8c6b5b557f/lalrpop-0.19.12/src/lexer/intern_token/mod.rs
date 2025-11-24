//! Generates an iterator type `Matcher` that looks roughly like

use crate::grammar::parse_tree::{InternToken, MatchMapping};
use crate::grammar::repr::{Grammar, TerminalLiteral};
use crate::lexer::re;
use crate::rust::RustWrite;
use std::io::{self, Write};

pub fn compile<W: Write>(
    grammar: &Grammar,
    intern_token: &InternToken,
    out: &mut RustWrite<W>,
) -> io::Result<()> {
    let prefix = &grammar.prefix;

    rust!(out, "#[cfg_attr(rustfmt, rustfmt_skip)]");
    rust!(out, "mod {}intern_token {{", prefix);
    rust!(out, "#![allow(unused_imports)]");
    out.write_uses("super::", grammar)?;
    rust!(
        out,
        "pub fn new_builder() -> {}lalrpop_util::lexer::MatcherBuilder {{",
        prefix
    );

    // create a vector of rust string literals with the text of each
    // regular expression
    let regex_strings = intern_token
        .match_entries
        .iter()
        .map(|match_entry| {
            (
                match match_entry.match_literal {
                    TerminalLiteral::Quoted(ref s) => re::parse_literal(s),
                    TerminalLiteral::Regex(ref s) => re::parse_regex(s).unwrap(),
                },
                match match_entry.user_name {
                    MatchMapping::Terminal(_) => false,
                    MatchMapping::Skip => true,
                },
            )
        })
        .map(|(regex, skip)| {
            // make sure all regex are anchored at the beginning of the input
            (format!("^({})", regex), skip)
        })
        .map(|(regex_str, skip)| {
            // create a rust string with text of the regex; the Debug impl
            // will add quotes and escape
            (format!("{:?}", regex_str), skip)
        });

    let mut contains_skip = false;

    rust!(out, "let {}strs: &[(&str, bool)] = &[", prefix);
    for (literal, skip) in regex_strings {
        rust!(out, "({}, {}),", literal, skip);
        contains_skip |= skip;
    }

    if !contains_skip {
        rust!(out, r#"(r"^(\s*)", true),"#);
    }

    rust!(out, "];");

    rust!(
        out,
        "{p}lalrpop_util::lexer::MatcherBuilder::new({p}strs.iter().copied()).unwrap()",
        p = prefix
    );

    rust!(out, "}}"); // fn
    rust!(out, "}}"); // mod
    Ok(())
}
