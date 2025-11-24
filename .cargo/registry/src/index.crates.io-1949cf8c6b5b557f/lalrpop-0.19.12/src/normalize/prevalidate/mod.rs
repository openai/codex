//! Validate checks some basic safety conditions.

use super::norm_util::{self, Symbols};
use super::precedence;
use super::{NormError, NormResult};

use crate::collections::{set, Multimap};
use crate::grammar::consts::*;
use crate::grammar::parse_tree::*;
use crate::grammar::repr as r;
use crate::util::Sep;
use string_cache::DefaultAtom as Atom;

#[cfg(test)]
mod test;

pub fn validate(grammar: &Grammar) -> NormResult<()> {
    let match_token: Option<&MatchToken> = grammar
        .items
        .iter()
        .filter_map(GrammarItem::as_match_token)
        .next();

    let extern_token: Option<&ExternToken> = grammar
        .items
        .iter()
        .filter_map(GrammarItem::as_extern_token)
        .next();

    let validator = Validator {
        grammar,
        match_token,
        extern_token,
    };

    validator.validate()
}

struct Validator<'grammar> {
    grammar: &'grammar Grammar,
    match_token: Option<&'grammar MatchToken>,
    extern_token: Option<&'grammar ExternToken>,
}

impl<'grammar> Validator<'grammar> {
    fn validate(&self) -> NormResult<()> {
        let allowed_names = vec![
            Atom::from(LALR),
            Atom::from(TABLE_DRIVEN),
            Atom::from(RECURSIVE_ASCENT),
            Atom::from(TEST_ALL),
        ];
        for annotation in &self.grammar.annotations {
            if !allowed_names.contains(&annotation.id) {
                return_err!(
                    annotation.id_span,
                    "unrecognized annotation `{}`",
                    annotation.id
                );
            }
        }

        for item in &self.grammar.items {
            match *item {
                GrammarItem::Use(..) => {}

                GrammarItem::MatchToken(ref data) => {
                    if data.span != self.match_token.unwrap().span {
                        return_err!(data.span, "multiple match definitions are not permitted");
                    }

                    // Only error if a custom lexer is specified, having a custom types is ok
                    if let Some(d) = self.extern_token {
                        if d.enum_token.is_some() {
                            return_err!(
                                d.span,
                                "extern (with custom tokens) and match definitions are mutually exclusive");
                        }
                    }

                    // Ensure that the catch all is final item of final block
                    for (contents_idx, match_contents) in data.contents.iter().enumerate() {
                        for (item_idx, item) in match_contents.items.iter().enumerate() {
                            if item.is_catch_all()
                                && (contents_idx != &data.contents.len() - 1
                                    || item_idx != &match_contents.items.len() - 1)
                            {
                                return_err!(item.span(), "Catch all must be final item");
                            }
                        }
                    }
                }

                GrammarItem::ExternToken(ref data) => {
                    if data.span != self.extern_token.unwrap().span {
                        return_err!(data.span, "multiple extern definitions are not permitted");
                    }

                    // Only error if a custom lexer is specified, having a custom types is ok
                    if let Some(d) = self.match_token {
                        if data.enum_token.is_some() {
                            return_err!(
                                d.span,
                                "match and extern (with custom tokens) definitions are mutually exclusive");
                        }
                    }

                    let allowed_names = vec![Atom::from(LOCATION), Atom::from(ERROR)];
                    let mut new_names = set();
                    for associated_type in &data.associated_types {
                        if !allowed_names.contains(&associated_type.type_name) {
                            return_err!(
                                associated_type.type_span,
                                "associated type `{}` not recognized, \
                                 try one of the following: {}",
                                associated_type.type_name,
                                Sep(", ", &allowed_names)
                            );
                        } else if !new_names.insert(associated_type.type_name.clone()) {
                            return_err!(
                                associated_type.type_span,
                                "associated type `{}` already specified",
                                associated_type.type_name
                            );
                        }
                    }
                }
                GrammarItem::Nonterminal(ref data) => {
                    if data.visibility.is_pub() && !data.args.is_empty() {
                        return_err!(data.span, "macros cannot be marked public");
                    }
                    let inline_annotation = Atom::from(INLINE);
                    let cfg_annotation = Atom::from(CFG);
                    let known_annotations = [inline_annotation.clone(), cfg_annotation.clone()];
                    let mut found_annotations = set();
                    for annotation in &data.annotations {
                        if !known_annotations.contains(&annotation.id) {
                            return_err!(
                                annotation.id_span,
                                "unrecognized annotation `{}`",
                                annotation.id
                            );
                        } else if !found_annotations.insert(annotation.id.clone()) {
                            return_err!(
                                annotation.id_span,
                                "duplicate annotation `{}`",
                                annotation.id
                            );
                        } else if annotation.id == inline_annotation && data.visibility.is_pub() {
                            return_err!(
                                annotation.id_span,
                                "public items cannot be marked #[inline]"
                            );
                        } else if annotation.id == cfg_annotation {
                            if data.visibility.is_pub() {
                                match annotation.arg {
                                    Some((ref name, _)) if name == "feature" => (),
                                    _ => return_err!(
                                        annotation.id_span,
                                        r#"`cfg` annotations must have a `feature = "my_feature" argument"#
                                    ),
                                }
                            } else {
                                return_err!(
                                    annotation.id_span,
                                    "private items cannot be marked #[cfg]"
                                );
                            }
                        }
                    }

                    self.validate_precedence(&data.alternatives)?;

                    for alternative in &data.alternatives {
                        self.validate_alternative(alternative)?;
                    }
                }
                GrammarItem::InternToken(..) => {}
            }
        }
        Ok(())
    }

    fn validate_precedence(&self, alternatives: &Vec<Alternative>) -> NormResult<()> {
        let with_precedence = alternatives.iter().any(|alt| {
            alt.annotations.iter().any(|ann| {
                ann.id == *precedence::PREC_ANNOT
                    || ann.id == *precedence::ASSOC_ANNOT
            })
        });

        if alternatives.is_empty() || !with_precedence {
            return Ok(());
        }

        // Used to check the absence of associativity annotations at the minimum level.
        let mut min_lvl = u32::MAX;
        let mut min_prec_ann: Option<&Annotation> = None;

        // Check that at least the first alternative has a precedence annotation
        alternatives
            .first()
            .map(|first| {
                let ann_prec_opt = first
                    .annotations
                    .iter()
                    .find(|ann| ann.id == *precedence::PREC_ANNOT);

                if ann_prec_opt.is_none() {
                    return_err!(
                        first.span,
                        "missing precedence annotation on the first alternative"
                    );
                } else {
                    Ok(())
                }
            })
            .transpose()?;

        // Check that annotations are well-formed
        alternatives.iter().try_for_each(|alt| {
            let ann_prec_opt = alt.annotations.iter().find(|ann| ann.id == *precedence::PREC_ANNOT);
            let ann_assoc_opt = alt.annotations.iter().find(|ann| ann.id == *precedence::ASSOC_ANNOT);

            if let Some(ann_prec) = ann_prec_opt {
                match &ann_prec.arg {
                    Some((name, value)) if *name == Atom::from(precedence::LVL_ARG) => {
                        if let Ok(lvl) = value.parse::<u32>() {
                            if lvl < min_lvl {
                                min_lvl = lvl;
                                min_prec_ann = ann_assoc_opt;
                            }
                            else if lvl == min_lvl && min_prec_ann.is_none() && ann_assoc_opt.is_some() {
                                min_prec_ann = ann_assoc_opt;
                            }
                        }
                        else {
                            return_err!(ann_prec.id_span, "could not parse the precedence level `{}`, expected integer", value);
                        }
                    }
                    Some((name, _)) => return_err!(ann_prec.id_span, "invalid argument `{}` for precedence annotation, expected `{}`", name, precedence::LVL_ARG),
                    None => return_err!(ann_prec.id_span, "missing argument for precedence annotation, expected `{}`", precedence::LVL_ARG),
                }
            }

            if let Some(ann_assoc) = ann_assoc_opt {
                match &ann_assoc.arg {
                    Some((name, value)) if *name == Atom::from(precedence::SIDE_ARG) => {
                        if value.parse::<precedence::Assoc>().is_err() {
                            return_err!(ann_assoc.id_span, "could not parse the associativity `{}`, expected `left`, `right`, `none` or `all`", value);
                        }
                    }
                    Some((name, _)) => return_err!(ann_assoc.id_span, "invalid argument `{}` for associativity annotation, expected `{}`", name, precedence::SIDE_ARG),
                    None => return_err!(ann_assoc.id_span, "missing argument for associativity annotation, expected `{}`", precedence::SIDE_ARG),
                }
            }

            Ok(())
        })?;

        if let Some(ann) = min_prec_ann {
            return_err!(
                ann.id_span,
                "cannot set associativity on the first precedence level {}",
                min_lvl
            );
        }

        Ok(())
    }

    fn validate_alternative(&self, alternative: &Alternative) -> NormResult<()> {
        self.validate_expr(&alternative.expr)?;

        let allowed_names = vec![
            Atom::from(precedence::PREC_ANNOT),
            Atom::from(precedence::ASSOC_ANNOT),
        ];

        for annotation in &alternative.annotations {
            if !allowed_names.contains(&annotation.id) {
                return_err!(
                    annotation.id_span,
                    "unrecognized annotation `{}`",
                    annotation.id
                );
            }
        }

        match norm_util::analyze_expr(&alternative.expr) {
            Symbols::Named(syms) => {
                if alternative.action.is_none() {
                    let sym = syms.iter().map(|&(_, _, sym)| sym).next().unwrap();
                    return_err!(
                        sym.span,
                        "named symbols (like `{}`) require a custom action",
                        sym
                    );
                }
            }
            Symbols::Anon(_) => {
                let empty_string = "".to_string();
                let action = {
                    match alternative.action {
                        Some(ActionKind::User(ref action)) => action,
                        Some(ActionKind::Fallible(ref action)) => action,
                        _ => &empty_string,
                    }
                };
                if norm_util::check_between_braces(action).is_in_curly_brackets() {
                    return_err!(
                        alternative.span,
                        "Using `<>` between curly braces (e.g., `{{<>}}`) only works when your parsed values have been given names (e.g., `<x:Foo>`, not just `<Foo>`)");
                }
            }
        }

        Ok(())
    }

    fn validate_expr(&self, expr: &ExprSymbol) -> NormResult<()> {
        for symbol in &expr.symbols {
            self.validate_symbol(symbol)?;
        }

        let chosen: Vec<&Symbol> = expr
            .symbols
            .iter()
            .filter(|sym| match sym.kind {
                SymbolKind::Choose(_) => true,
                _ => false,
            })
            .collect();

        let named: Multimap<Atom, Vec<&Symbol>> = expr
            .symbols
            .iter()
            .filter_map(|sym| match sym.kind {
                SymbolKind::Name(ref nt, _) => Some((nt.name.clone(), sym)),
                _ => None,
            })
            .collect();

        if !chosen.is_empty() && !named.is_empty() {
            return_err!(
                chosen[0].span,
                "anonymous symbols like this one cannot be combined with \
                 named symbols like `{}`",
                named.into_iter().next().unwrap().1[0]
            );
        }

        for (name, syms) in named.into_iter() {
            if syms.len() > 1 {
                return_err!(
                    syms[1].span,
                    "multiple symbols named `{}` are not permitted",
                    name
                );
            }
        }

        Ok(())
    }

    fn validate_symbol(&self, symbol: &Symbol) -> NormResult<()> {
        match symbol.kind {
            SymbolKind::Expr(ref expr) => {
                self.validate_expr(expr)?;
            }
            SymbolKind::AmbiguousId(_) => { /* see resolve */ }
            SymbolKind::Terminal(_) => { /* see postvalidate! */ }
            SymbolKind::Nonterminal(_) => { /* see resolve */ }
            SymbolKind::Error => {
                let mut algorithm = r::Algorithm::default();
                read_algorithm(&self.grammar.annotations, &mut algorithm);
                if algorithm.codegen == r::LrCodeGeneration::RecursiveAscent {
                    return_err!(
                        symbol.span,
                        "error recovery is not yet supported by recursive ascent parsers"
                    );
                }
            }
            SymbolKind::Macro(ref msym) => {
                debug_assert!(!msym.args.is_empty());
                for arg in &msym.args {
                    self.validate_symbol(arg)?;
                }
            }
            SymbolKind::Repeat(ref repeat) => {
                self.validate_symbol(&repeat.symbol)?;
            }
            SymbolKind::Choose(ref sym) | SymbolKind::Name(_, ref sym) => {
                self.validate_symbol(sym)?;
            }
            SymbolKind::Lookahead | SymbolKind::Lookbehind => {
                // if using an internal tokenizer, lookahead/lookbehind are ok.
                if let Some(extern_token) = self.extern_token {
                    if extern_token.enum_token.is_some() {
                        // otherwise, the Location type must be specified.
                        let loc = Atom::from(LOCATION);
                        if self.extern_token.unwrap().associated_type(loc).is_none() {
                            return_err!(
                                symbol.span,
                                "lookahead/lookbehind require you to declare the type of \
                                 a location; add a `type {} = ..` statement to the extern token \
                                 block",
                                LOCATION
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
