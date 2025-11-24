use crate::grammar::consts::INLINE;
use crate::grammar::parse_tree::{
    ActionKind, Alternative, Annotation, Condition, ConditionOp, ExprSymbol, Grammar, GrammarItem,
    MacroSymbol, Name, NonterminalData, NonterminalString, Path, RepeatOp, RepeatSymbol, Span,
    Symbol, SymbolKind, TerminalLiteral, TerminalString, TypeRef, Visibility,
};
use crate::normalize::norm_util::{self, Symbols};
use crate::normalize::resolve;
use crate::normalize::{NormError, NormResult};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::mem;
use string_cache::DefaultAtom as Atom;

#[cfg(test)]
mod test;

pub fn expand_macros(input: Grammar) -> NormResult<Grammar> {
    let input = resolve::resolve(input)?;

    let items = input.items;

    let (macro_defs, mut items): (Vec<_>, Vec<_>) =
        items.into_iter().partition(GrammarItem::is_macro_def);

    let macro_defs: HashMap<_, _> = macro_defs
        .into_iter()
        .map(|md| match md {
            GrammarItem::Nonterminal(ref data) => (data.name.clone(), data.clone()),
            _ => unreachable!(),
        })
        .collect();

    let mut expander = MacroExpander::new(macro_defs);
    expander.expand(&mut items)?;

    Ok(Grammar { items, ..input })
}

struct MacroExpander {
    macro_defs: HashMap<NonterminalString, NonterminalData>,
    expansion_set: HashSet<NonterminalString>,
    expansion_stack: Vec<Symbol>,
}

impl MacroExpander {
    fn new(macro_defs: HashMap<NonterminalString, NonterminalData>) -> MacroExpander {
        MacroExpander {
            macro_defs,
            expansion_stack: Vec::new(),
            expansion_set: HashSet::new(),
        }
    }

    fn expand(&mut self, items: &mut Vec<GrammarItem>) -> NormResult<()> {
        let mut counter = 0;
        loop {
            // Find any macro uses in items added since last round and
            // replace them in place with the expanded version:
            for item in &mut items[counter..] {
                self.replace_item(item);
            }
            counter = items.len();

            // No more expansion to do.
            if self.expansion_stack.is_empty() {
                return Ok(());
            }

            // Drain expansion stack:
            while let Some(sym) = self.expansion_stack.pop() {
                match sym.kind {
                    SymbolKind::Macro(msym) => {
                        items.push(self.expand_macro_symbol(sym.span, msym)?)
                    }
                    SymbolKind::Expr(expr) => items.push(self.expand_expr_symbol(sym.span, expr)?),
                    SymbolKind::Repeat(repeat) => {
                        items.push(self.expand_repeat_symbol(sym.span, *repeat)?)
                    }
                    SymbolKind::Lookahead => items.push(self.expand_lookaround_symbol(
                        sym.span,
                        "@L",
                        ActionKind::Lookahead,
                    )?),
                    SymbolKind::Lookbehind => items.push(self.expand_lookaround_symbol(
                        sym.span,
                        "@R",
                        ActionKind::Lookbehind,
                    )?),
                    _ => panic!("don't know how to expand `{:?}`", sym),
                }
            }
        }
    }

    fn replace_item(&mut self, item: &mut GrammarItem) {
        match *item {
            GrammarItem::MatchToken(..) => {}
            GrammarItem::ExternToken(..) => {}
            GrammarItem::InternToken(..) => {}
            GrammarItem::Use(..) => {}
            GrammarItem::Nonterminal(ref mut data) => {
                // Should not encounter macro definitions here,
                // they've already been siphoned off.
                assert!(!data.is_macro_def());

                for alternative in &mut data.alternatives {
                    self.replace_symbols(&mut alternative.expr.symbols);
                }
            }
        }
    }

    fn replace_symbols(&mut self, symbols: &mut [Symbol]) {
        for symbol in symbols {
            self.replace_symbol(symbol);
        }
    }

    fn replace_symbol(&mut self, symbol: &mut Symbol) {
        match symbol.kind {
            SymbolKind::AmbiguousId(ref id) => {
                panic!("ambiguous id `{}` encountered after name resolution", id)
            }
            SymbolKind::Macro(ref mut m) => {
                for sym in &mut m.args {
                    self.replace_symbol(sym);
                }
            }
            SymbolKind::Expr(ref mut expr) => {
                self.replace_symbols(&mut expr.symbols);
            }
            SymbolKind::Repeat(ref mut repeat) => {
                self.replace_symbol(&mut repeat.symbol);
            }
            SymbolKind::Terminal(_) | SymbolKind::Nonterminal(_) | SymbolKind::Error => {
                return;
            }
            SymbolKind::Choose(ref mut sym) | SymbolKind::Name(_, ref mut sym) => {
                self.replace_symbol(sym);
                return;
            }
            SymbolKind::Lookahead | SymbolKind::Lookbehind => {}
        }

        // only symbols we intend to expand fallthrough to here

        let key = NonterminalString(Atom::from(symbol.canonical_form()));
        let replacement = Symbol {
            span: symbol.span,
            kind: SymbolKind::Nonterminal(key.clone()),
        };
        let to_expand = mem::replace(symbol, replacement);
        if self.expansion_set.insert(key) {
            self.expansion_stack.push(to_expand);
        }
    }

    ///////////////////////////////////////////////////////////////////////////
    // Macro expansion

    fn expand_macro_symbol(&mut self, span: Span, msym: MacroSymbol) -> NormResult<GrammarItem> {
        let msym_name = NonterminalString(Atom::from(msym.canonical_form()));

        let mdef = match self.macro_defs.get(&msym.name) {
            Some(v) => v,
            None => return_err!(span, "no macro definition found for `{}`", msym.name),
        };

        if mdef.args.len() != msym.args.len() {
            return_err!(
                span,
                "expected {} arguments to `{}` but found {}",
                mdef.args.len(),
                msym.name,
                msym.args.len()
            );
        }

        let args: HashMap<NonterminalString, SymbolKind> = mdef
            .args
            .iter()
            .cloned()
            .zip(msym.args.into_iter().map(|s| s.kind))
            .collect();

        let type_decl = mdef
            .type_decl
            .as_ref()
            .map(|tr| self.macro_expand_type_ref(&args, tr));

        // due to the use of `?`, it's a bit awkward to write this with an iterator
        let mut alternatives: Vec<Alternative> = vec![];

        for alternative in &mdef.alternatives {
            if !self.evaluate_cond(&args, &alternative.condition)? {
                continue;
            }
            alternatives.push(Alternative {
                span,
                expr: self.macro_expand_expr_symbol(&args, &alternative.expr),
                condition: None,
                action: alternative.action.clone(),
                annotations: alternative.annotations.clone(),
            });
        }

        Ok(GrammarItem::Nonterminal(NonterminalData {
            visibility: mdef.visibility.clone(),
            span,
            name: msym_name,
            annotations: mdef.annotations.clone(),
            args: vec![],
            type_decl,
            alternatives,
        }))
    }

    fn macro_expand_type_refs(
        &self,
        args: &HashMap<NonterminalString, SymbolKind>,
        type_refs: &[TypeRef],
    ) -> Vec<TypeRef> {
        type_refs
            .iter()
            .map(|tr| self.macro_expand_type_ref(args, tr))
            .collect()
    }

    fn macro_expand_type_ref(
        &self,
        args: &HashMap<NonterminalString, SymbolKind>,
        type_ref: &TypeRef,
    ) -> TypeRef {
        match *type_ref {
            TypeRef::Tuple(ref trs) => TypeRef::Tuple(self.macro_expand_type_refs(args, trs)),
            TypeRef::Slice(ref tr) => {
                TypeRef::Slice(Box::new(self.macro_expand_type_ref(args, tr)))
            }
            TypeRef::Nominal {
                ref path,
                ref types,
            } => TypeRef::Nominal {
                path: path.clone(),
                types: self.macro_expand_type_refs(args, types),
            },
            TypeRef::Lifetime(ref id) => TypeRef::Lifetime(id.clone()),
            TypeRef::OfSymbol(ref sym) => TypeRef::OfSymbol(sym.clone()),
            TypeRef::Ref {
                ref lifetime,
                mutable,
                ref referent,
            } => TypeRef::Ref {
                lifetime: lifetime.clone(),
                mutable,
                referent: Box::new(self.macro_expand_type_ref(args, referent)),
            },
            TypeRef::Id(ref id) => match args.get(&NonterminalString(id.clone())) {
                Some(sym) => TypeRef::OfSymbol(sym.clone()),
                None => TypeRef::Nominal {
                    path: Path::from_id(id.clone()),
                    types: vec![],
                },
            },
            TypeRef::TraitObject {
                ref path,
                ref types,
            } => TypeRef::TraitObject {
                path: path.clone(),
                types: self.macro_expand_type_refs(args, types),
            },
            TypeRef::Fn {
                ref forall,
                ref path,
                ref parameters,
                ref ret,
            } => TypeRef::Fn {
                forall: forall.clone(),
                path: path.clone(),
                parameters: self.macro_expand_type_refs(args, parameters),
                ret: ret
                    .as_ref()
                    .map(|t| Box::new(self.macro_expand_type_ref(args, t))),
            },
        }
    }

    fn evaluate_cond(
        &self,
        args: &HashMap<NonterminalString, SymbolKind>,
        opt_cond: &Option<Condition>,
    ) -> NormResult<bool> {
        if let Some(ref c) = *opt_cond {
            match args[&c.lhs] {
                SymbolKind::Terminal(TerminalString::Literal(TerminalLiteral::Quoted(ref lhs))) => {
                    match c.op {
                        ConditionOp::Equals => Ok(lhs == &c.rhs),
                        ConditionOp::NotEquals => Ok(lhs != &c.rhs),
                        ConditionOp::Match => self.re_match(c.span, lhs, &c.rhs),
                        ConditionOp::NotMatch => Ok(!self.re_match(c.span, lhs, &c.rhs)?),
                    }
                }
                ref lhs => {
                    return_err!(
                        c.span,
                        "invalid condition LHS `{}`, expected a string literal, not `{}`",
                        c.lhs,
                        lhs
                    );
                }
            }
        } else {
            Ok(true)
        }
    }

    fn re_match(&self, span: Span, lhs: &Atom, regex: &Atom) -> NormResult<bool> {
        let re = match Regex::new(regex) {
            Ok(re) => re,
            Err(err) => return_err!(span, "invalid regular expression `{}`: {}", regex, err),
        };
        Ok(re.is_match(lhs))
    }

    fn macro_expand_symbols(
        &self,
        args: &HashMap<NonterminalString, SymbolKind>,
        expr: &[Symbol],
    ) -> Vec<Symbol> {
        expr.iter()
            .map(|s| self.macro_expand_symbol(args, s))
            .collect()
    }

    fn macro_expand_expr_symbol(
        &self,
        args: &HashMap<NonterminalString, SymbolKind>,
        expr: &ExprSymbol,
    ) -> ExprSymbol {
        ExprSymbol {
            symbols: self.macro_expand_symbols(args, &expr.symbols),
        }
    }

    fn macro_expand_symbol(
        &self,
        args: &HashMap<NonterminalString, SymbolKind>,
        symbol: &Symbol,
    ) -> Symbol {
        let kind = match symbol.kind {
            SymbolKind::Expr(ref expr) => {
                SymbolKind::Expr(self.macro_expand_expr_symbol(args, expr))
            }
            SymbolKind::Terminal(ref id) => SymbolKind::Terminal(id.clone()),
            SymbolKind::Nonterminal(ref id) => match args.get(id) {
                Some(sym) => sym.clone(),
                None => SymbolKind::Nonterminal(id.clone()),
            },
            SymbolKind::Macro(ref msym) => SymbolKind::Macro(MacroSymbol {
                name: msym.name.clone(),
                args: self.macro_expand_symbols(args, &msym.args),
            }),
            SymbolKind::Repeat(ref r) => SymbolKind::Repeat(Box::new(RepeatSymbol {
                op: r.op,
                symbol: self.macro_expand_symbol(args, &r.symbol),
            })),
            SymbolKind::Choose(ref sym) => {
                SymbolKind::Choose(Box::new(self.macro_expand_symbol(args, sym)))
            }
            SymbolKind::Name(ref id, ref sym) => SymbolKind::Name(
                Name::new(id.mutable, id.name.clone()),
                Box::new(self.macro_expand_symbol(args, sym)),
            ),
            SymbolKind::Lookahead => SymbolKind::Lookahead,
            SymbolKind::Lookbehind => SymbolKind::Lookbehind,
            SymbolKind::Error => SymbolKind::Error,
            SymbolKind::AmbiguousId(ref id) => {
                panic!("ambiguous id `{}` encountered after name resolution", id)
            }
        };

        Symbol {
            span: symbol.span,
            kind,
        }
    }

    ///////////////////////////////////////////////////////////////////////////
    // Expr expansion

    fn expand_expr_symbol(&mut self, span: Span, expr: ExprSymbol) -> NormResult<GrammarItem> {
        let name = NonterminalString(Atom::from(expr.canonical_form()));

        let (action, ty_ref) =
            match norm_util::analyze_expr(&expr) {
                Symbols::Named(names) => {
                    let (_, ref ex_id, ex_sym) = names[0];
                    return_err!(
                    span,
                    "named symbols like `{}:{}` are only allowed at the top-level of a nonterminal",
                    ex_id, ex_sym)
                }
                Symbols::Anon(syms) => (
                    if syms.len() == 1 {
                        action("<>")
                    } else {
                        action("(<>)")
                    },
                    maybe_tuple(
                        syms.into_iter()
                            .map(|(_, s)| TypeRef::OfSymbol(s.kind.clone()))
                            .collect(),
                    ),
                ),
            };

        Ok(GrammarItem::Nonterminal(NonterminalData {
            visibility: Visibility::Priv,
            span,
            name,
            annotations: inline(span),
            args: vec![],
            type_decl: Some(ty_ref),
            alternatives: vec![Alternative {
                span,
                expr,
                condition: None,
                action,
                annotations: Vec::new(),
            }],
        }))
    }

    ///////////////////////////////////////////////////////////////////////////
    // Expr expansion

    fn expand_repeat_symbol(
        &mut self,
        span: Span,
        repeat: RepeatSymbol,
    ) -> NormResult<GrammarItem> {
        let name = NonterminalString(Atom::from(repeat.canonical_form()));
        let v = Atom::from("v");
        let e = Atom::from("e");

        let base_symbol_ty = TypeRef::OfSymbol(repeat.symbol.kind.clone());

        match repeat.op {
            RepeatOp::Star => {
                let path = Path::vec();
                let ty_ref = TypeRef::Nominal {
                    path,
                    types: vec![base_symbol_ty],
                };

                let plus_repeat = Box::new(RepeatSymbol {
                    op: RepeatOp::Plus,
                    symbol: repeat.symbol,
                });

                Ok(GrammarItem::Nonterminal(NonterminalData {
                    visibility: Visibility::Priv,
                    span,
                    name,
                    annotations: inline(span),
                    args: vec![],
                    type_decl: Some(ty_ref),
                    alternatives: vec![
                        // X* =
                        Alternative {
                            span,
                            expr: ExprSymbol { symbols: vec![] },
                            condition: None,
                            action: action("alloc::vec![]"),
                            annotations: vec![],
                        },
                        // X* = <v:X+>
                        Alternative {
                            span,
                            expr: ExprSymbol {
                                symbols: vec![Symbol::new(
                                    span,
                                    SymbolKind::Name(
                                        Name::immut(v),
                                        Box::new(Symbol::new(
                                            span,
                                            SymbolKind::Repeat(plus_repeat),
                                        )),
                                    ),
                                )],
                            },
                            condition: None,
                            action: action("v"),
                            annotations: vec![],
                        },
                    ],
                }))
            }

            RepeatOp::Plus => {
                let path = Path::vec();
                let ty_ref = TypeRef::Nominal {
                    path,
                    types: vec![base_symbol_ty],
                };

                Ok(GrammarItem::Nonterminal(NonterminalData {
                    visibility: Visibility::Priv,
                    span,
                    name: name.clone(),
                    annotations: vec![],
                    args: vec![],
                    type_decl: Some(ty_ref),
                    alternatives: vec![
                        // X+ = X
                        Alternative {
                            span,
                            expr: ExprSymbol {
                                symbols: vec![repeat.symbol.clone()],
                            },
                            condition: None,
                            action: action("alloc::vec![<>]"),
                            annotations: vec![],
                        },
                        // X+ = <v:X+> <e:X>
                        Alternative {
                            span,
                            expr: ExprSymbol {
                                symbols: vec![
                                    Symbol::new(
                                        span,
                                        SymbolKind::Name(
                                            Name::immut(v),
                                            Box::new(Symbol::new(
                                                span,
                                                SymbolKind::Nonterminal(name),
                                            )),
                                        ),
                                    ),
                                    Symbol::new(
                                        span,
                                        SymbolKind::Name(
                                            Name::immut(e),
                                            Box::new(repeat.symbol),
                                        ),
                                    ),
                                ],
                            },
                            condition: None,
                            action: action("{ let mut v = v; v.push(e); v }"),
                            annotations: vec![],
                        },
                    ],
                }))
            }

            RepeatOp::Question => {
                let path = Path::option();
                let ty_ref = TypeRef::Nominal {
                    path,
                    types: vec![base_symbol_ty],
                };

                Ok(GrammarItem::Nonterminal(NonterminalData {
                    visibility: Visibility::Priv,
                    span,
                    name,
                    annotations: inline(span),
                    args: vec![],
                    type_decl: Some(ty_ref),
                    alternatives: vec![
                        // X? = X => Some(<>)
                        Alternative {
                            span,
                            expr: ExprSymbol {
                                symbols: vec![repeat.symbol],
                            },
                            condition: None,
                            action: action("Some(<>)"),
                            annotations: vec![],
                        },
                        // X? = { => None; }
                        Alternative {
                            span,
                            expr: ExprSymbol { symbols: vec![] },
                            condition: None,
                            action: action("None"),
                            annotations: vec![],
                        },
                    ],
                }))
            }
        }
    }

    fn expand_lookaround_symbol(
        &mut self,
        span: Span,
        name: &str,
        action: ActionKind,
    ) -> NormResult<GrammarItem> {
        let name = NonterminalString(Atom::from(name));
        Ok(GrammarItem::Nonterminal(NonterminalData {
            visibility: Visibility::Priv,
            span,
            name,
            annotations: inline(span),
            args: vec![],
            type_decl: None,
            alternatives: vec![Alternative {
                span,
                expr: ExprSymbol { symbols: vec![] },
                condition: None,
                action: Some(action),
                annotations: vec![],
            }],
        }))
    }
}

fn maybe_tuple(v: Vec<TypeRef>) -> TypeRef {
    if v.len() == 1 {
        v.into_iter().next().unwrap()
    } else {
        TypeRef::Tuple(v)
    }
}

fn action(s: &str) -> Option<ActionKind> {
    Some(ActionKind::User(s.to_string()))
}

fn inline(span: Span) -> Vec<Annotation> {
    vec![Annotation {
        id_span: span,
        id: Atom::from(INLINE),
        arg: None,
    }]
}
