//! If an extern token is provided, then this pass validates that
//! terminal IDs have conversions. Otherwise, it generates a
//! tokenizer. This can only be done after macro expansion because
//! some macro arguments never make it into an actual production and
//! are only used in `if` conditions; we use string literals for
//! those, but they do not have to have a defined conversion.

use super::{NormError, NormResult};

use crate::collections::{Map, Set};
use crate::grammar::consts::*;
use crate::grammar::parse_tree::*;
use crate::lexer::dfa::{self, DFAConstructionError, Precedence};
use crate::lexer::nfa::NFAConstructionError::*;
use crate::lexer::re;
use string_cache::DefaultAtom as Atom;

#[cfg(test)]
mod test;

pub fn validate(mut grammar: Grammar) -> NormResult<Grammar> {
    let mode = {
        let mode = if let Some(enum_token) = grammar.enum_token() {
            assert!(
                grammar.match_token().is_none(),
                "validator permitted both an extern/match section"
            );

            TokenMode::Extern {
                conversions: enum_token
                    .conversions
                    .iter()
                    .map(|conversion| conversion.from.clone())
                    .collect(),
            }
        } else {
            TokenMode::Internal {
                match_block: MatchBlock::new(grammar.match_token())?,
            }
        };

        let mut validator = Validator {
            grammar: &grammar,
            mode,
        };

        validator.validate()?;

        validator.mode
    };

    match mode {
        TokenMode::Extern { .. } => {
            // If using an external tokenizer, we're all done at this point.
        }
        TokenMode::Internal { match_block } => {
            // Otherwise, construct the `InternToken` item.
            construct(&mut grammar, match_block)?;
        }
    }

    Ok(grammar)
}

///////////////////////////////////////////////////////////////////////////
// Validation phase -- this phase walks the grammar and visits all
// terminals. If using an external set of tokens, it checks that all
// terminals have a defined conversion to some pattern. Otherwise,
// it collects all terminals into the `all_literals` set for later use.

struct Validator<'grammar> {
    grammar: &'grammar Grammar,
    mode: TokenMode,
}

enum TokenMode {
    /// If there is an `extern { ... }` section that defines
    /// conversions of the form `TERMINAL => PATTERN`, then this is a
    /// set of those terminals. These are the only terminals that the
    /// user should be using.
    Extern { conversions: Set<TerminalString> },

    /// Otherwise, we are synthesizing the tokenizer. In that case,
    /// `match_block` summarizes the data from the `match { ... }`
    /// section, if any. If there was no `match` section, or the
    /// section contains a wildcard, the user can also use additional
    /// terminals in the grammar.
    Internal { match_block: MatchBlock },
}

/// Data summarizing the `match { }` block, along with any literals we
/// scraped up.
#[derive(Default)]
struct MatchBlock {
    /// This map stores the `match { }` entries. If `match_catch_all`
    /// is true, then we will grow this set with "identity mappings"
    /// for new literals that we find.
    match_entries: Vec<MatchEntry>,

    /// The names of all terminals the user can legally type. If
    /// `match_catch_all` is true, then if we encounter additional
    /// terminal literals in the grammar, we will add them to this
    /// set.
    match_user_names: Set<TerminalString>,

    /// For each terminal literal that we have to match, the span
    /// where it appeared in user's source.  This can either be in the
    /// `match { }` section or else in the grammar somewhere (if added
    /// due to a catch-all, or there is no match section).
    spans: Map<TerminalLiteral, Span>,

    /// True if we should permit unrecognized literals to be used.
    catch_all: bool,
}

impl MatchBlock {
    /// Creates a `MatchBlock` by reading the data out of the `match {
    /// ... }` block that the user provided (if any).
    fn new(opt_match_token: Option<&MatchToken>) -> NormResult<Self> {
        let mut match_block = Self::default();
        if let Some(match_token) = opt_match_token {
            for (idx, mc) in match_token.contents.iter().enumerate() {
                let precedence = match_token.contents.len() - idx;
                for item in &mc.items {
                    match *item {
                        MatchItem::Unmapped(ref sym, span) => {
                            match_block.add_match_entry(
                                precedence,
                                sym.clone(),
                                MatchMapping::Terminal(TerminalString::Literal(sym.clone())),
                                span,
                            )?;
                        }
                        MatchItem::Mapped(ref sym, ref user, span) => {
                            match_block.add_match_entry(
                                precedence,
                                sym.clone(),
                                user.clone(),
                                span,
                            )?;
                        }
                        MatchItem::CatchAll(_) => {
                            match_block.catch_all = true;
                        }
                    }
                }
            }
        } else {
            // no match block is equivalent to `match { _ }`
            match_block.catch_all = true;
        }
        Ok(match_block)
    }

    fn add_match_entry(
        &mut self,
        match_group_precedence: usize,
        sym: TerminalLiteral,
        user_name: MatchMapping,
        span: Span,
    ) -> NormResult<()> {
        if let Some(_old_span) = self.spans.insert(sym.clone(), span) {
            return_err!(span, "multiple match entries for `{}`", sym);
        }

        // NB: It's legal for multiple regex to produce same terminal.
        if let MatchMapping::Terminal(user_name) = &user_name {
            self.match_user_names.insert(user_name.clone());
        }

        self.match_entries.push(MatchEntry {
            precedence: match_group_precedence * 2 + sym.base_precedence(),
            match_literal: sym,
            user_name,
        });
        Ok(())
    }

    fn add_literal_from_grammar(&mut self, sym: TerminalLiteral, span: Span) -> NormResult<()> {
        // Already saw this literal, maybe in a match entry, maybe in the grammar.
        if self
            .match_user_names
            .contains(&TerminalString::Literal(sym.clone()))
        {
            return Ok(());
        }

        if !self.catch_all {
            return_err!(
                span,
                "terminal `{}` does not have a match mapping defined for it",
                sym
            );
        }

        self.match_user_names
            .insert(TerminalString::Literal(sym.clone()));

        self.match_entries.push(MatchEntry {
            precedence: sym.base_precedence(),
            match_literal: sym.clone(),
            user_name: MatchMapping::Terminal(TerminalString::Literal(sym.clone())),
        });

        self.spans.insert(sym, span);

        Ok(())
    }
}

impl<'grammar> Validator<'grammar> {
    fn validate(&mut self) -> NormResult<()> {
        for item in &self.grammar.items {
            match *item {
                GrammarItem::Use(..) => {}
                GrammarItem::MatchToken(..) => {}
                GrammarItem::ExternToken(_) => {}
                GrammarItem::InternToken(_) => {}
                GrammarItem::Nonterminal(ref data) => {
                    for alternative in &data.alternatives {
                        self.validate_alternative(alternative)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_alternative(&mut self, alternative: &Alternative) -> NormResult<()> {
        assert!(alternative.condition.is_none()); // macro expansion should have removed these
        self.validate_expr(&alternative.expr)?;
        Ok(())
    }

    fn validate_expr(&mut self, expr: &ExprSymbol) -> NormResult<()> {
        for symbol in &expr.symbols {
            self.validate_symbol(symbol)?;
        }
        Ok(())
    }

    fn validate_symbol(&mut self, symbol: &Symbol) -> NormResult<()> {
        match symbol.kind {
            SymbolKind::Expr(ref expr) => {
                self.validate_expr(expr)?;
            }
            SymbolKind::Terminal(ref term) => {
                self.validate_terminal(symbol.span, term)?;
            }
            SymbolKind::Nonterminal(_) => {}
            SymbolKind::Repeat(ref repeat) => {
                self.validate_symbol(&repeat.symbol)?;
            }
            SymbolKind::Choose(ref sym) | SymbolKind::Name(_, ref sym) => {
                self.validate_symbol(sym)?;
            }
            SymbolKind::Lookahead | SymbolKind::Lookbehind | SymbolKind::Error => {}
            SymbolKind::AmbiguousId(ref id) => {
                panic!("ambiguous id `{}` encountered after name resolution", id)
            }
            SymbolKind::Macro(..) => {
                panic!("macro not removed: {:?}", symbol);
            }
        }

        Ok(())
    }

    fn validate_terminal(&mut self, span: Span, term: &TerminalString) -> NormResult<()> {
        match self.mode {
            // If there is an extern token definition, validate that
            // this terminal has a defined conversion.
            TokenMode::Extern { ref conversions } => {
                if !conversions.contains(term) {
                    return_err!(
                        span,
                        "terminal `{}` does not have a pattern defined for it",
                        term
                    );
                }
            }

            // If there is no extern token definition, then collect
            // the terminal literals ("class", r"[a-z]+") into a set.
            TokenMode::Internal {
                ref mut match_block,
            } => {
                match *term {
                    TerminalString::Bare(_) => assert!(
                        match_block.match_user_names.contains(term),
                        "bare terminal without match entry: {}",
                        term
                    ),

                    TerminalString::Literal(ref l) => {
                        match_block.add_literal_from_grammar(l.clone(), span)?
                    }

                    // Error is a builtin terminal that always exists
                    TerminalString::Error => (),
                }
            }
        }

        Ok(())
    }
}

///////////////////////////////////////////////////////////////////////////
// Construction phase -- if we are constructing a tokenizer, this
// phase builds up an internal token DFA.

fn construct(grammar: &mut Grammar, match_block: MatchBlock) -> NormResult<()> {
    let MatchBlock {
        mut match_entries,
        spans,
        ..
    } = match_block;

    // Sort match entries by order of increasing precedence.
    match_entries.sort();

    // Build up two vectors, one of parsed regular expressions and
    // one of precedences, that are parallel with `literals`.
    let mut regexs = Vec::with_capacity(match_entries.len());
    let mut precedences = Vec::with_capacity(match_entries.len());
    for match_entry in &match_entries {
        precedences.push(Precedence(match_entry.precedence));
        match match_entry.match_literal {
            TerminalLiteral::Quoted(ref s) => {
                regexs.push(re::parse_literal(s));
            }
            TerminalLiteral::Regex(ref s) => {
                match re::parse_regex(s) {
                    Ok(regex) => regexs.push(regex),
                    Err(error) => {
                        let literal_span = spans[&match_entry.match_literal];
                        // FIXME -- take offset into account for
                        // span; this requires knowing how many #
                        // the user used, which we do not track
                        return_err!(literal_span, "invalid regular expression: {}", error);
                    }
                }
            }
        }
    }

    let dfa = match dfa::build_dfa(&regexs, &precedences) {
        Ok(dfa) => dfa,
        Err(DFAConstructionError::NFAConstructionError { index, error }) => {
            let feature = match error {
                NamedCaptures => r#"named captures (`(?P<foo>...)`)"#,
                NonGreedy => r#""non-greedy" repetitions (`*?` or `+?`)"#,
                WordBoundary => r#"word boundaries (`\b` or `\B`)"#,
                LineBoundary => r#"line boundaries (`^` or `$`)"#,
                TextBoundary => r#"text boundaries (`^` or `$`)"#,
                ByteRegex => r#"byte-based matches"#,
            };
            let literal = &match_entries[index.index()].match_literal;
            return_err!(
                spans[literal],
                "{} are not supported in regular expressions",
                feature
            )
        }
        Err(DFAConstructionError::Ambiguity { match0, match1 }) => {
            let literal0 = &match_entries[match0.index()].match_literal;
            let literal1 = &match_entries[match1.index()].match_literal;
            // FIXME(#88) -- it'd be nice to give an example here
            return_err!(
                spans[literal0],
                "ambiguity detected between the terminal `{}` and the terminal `{}`",
                literal0,
                literal1
            )
        }
    };

    grammar
        .items
        .push(GrammarItem::InternToken(InternToken { match_entries, dfa }));

    // we need to inject a `'input` lifetime and `input: &'input str` parameter as well:

    let input_lifetime = Lifetime::input();
    for parameter in &grammar.type_parameters {
        match parameter {
            TypeParameter::Lifetime(i) if *i == input_lifetime => {
                return_err!(
                    grammar.span,
                    "since there is no external token enum specified, \
                     the `'input` lifetime is implicit and cannot be declared"
                );
            }
            _ => {}
        }
    }

    let input_parameter = Atom::from(INPUT_PARAMETER);
    for parameter in &grammar.parameters {
        if parameter.name == input_parameter {
            return_err!(
                grammar.span,
                "since there is no external token enum specified, \
                 the `input` parameter is implicit and cannot be declared"
            );
        }
    }

    grammar
        .type_parameters
        .insert(0, TypeParameter::Lifetime(input_lifetime.clone()));

    let parameter = Parameter {
        name: input_parameter,
        ty: TypeRef::Ref {
            lifetime: Some(input_lifetime),
            mutable: false,
            referent: Box::new(TypeRef::Id(Atom::from("str"))),
        },
    };
    grammar.parameters.push(parameter);

    Ok(())
}
