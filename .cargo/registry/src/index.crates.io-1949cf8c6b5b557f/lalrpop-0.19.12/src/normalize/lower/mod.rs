//! Lower
//!

use crate::collections::{map, Map};
use crate::grammar::consts::CFG;
use crate::grammar::parse_tree as pt;
use crate::grammar::parse_tree::{
    read_algorithm, GrammarItem, InternToken, Lifetime, MatchMapping, Name, NonterminalString,
    Path, TerminalString,
};
use crate::grammar::pattern::{Pattern, PatternKind};
use crate::grammar::repr as r;
use crate::normalize::norm_util::{self, Symbols};
use crate::normalize::NormResult;
use crate::session::Session;
use string_cache::DefaultAtom as Atom;

pub fn lower(session: &Session, grammar: pt::Grammar, types: r::Types) -> NormResult<r::Grammar> {
    let state = LowerState::new(session, types, &grammar);
    state.lower(grammar)
}

struct LowerState<'s> {
    session: &'s Session,
    prefix: String,
    action_fn_defns: Vec<r::ActionFnDefn>,
    nonterminals: Map<NonterminalString, r::NonterminalData>,
    conversions: Vec<(TerminalString, Pattern<r::TypeRepr>)>,
    intern_token: Option<InternToken>,
    types: r::Types,
    uses_error_recovery: bool,
}

impl<'s> LowerState<'s> {
    fn new(session: &'s Session, types: r::Types, grammar: &pt::Grammar) -> Self {
        LowerState {
            session,
            prefix: grammar.prefix.clone(),
            action_fn_defns: vec![],
            nonterminals: map(),
            conversions: vec![],
            types,
            intern_token: None,
            uses_error_recovery: false,
        }
    }

    fn lower(mut self, grammar: pt::Grammar) -> NormResult<r::Grammar> {
        let start_symbols = self.synthesize_start_symbols(&grammar);

        let mut uses = vec![];
        let mut token_span = None;
        let internal_token_path = Path {
            absolute: false,
            ids: vec![Atom::from("Token")],
        };

        for item in grammar.items {
            match item {
                pt::GrammarItem::Use(data) => {
                    uses.push(data);
                }

                pt::GrammarItem::MatchToken(_) => {
                    // The declarations in the match token are handled
                    // fully by the `token_check` when it constructs the
                    //  `InternToken` -- there is nothing left to do here.
                }

                pt::GrammarItem::InternToken(data) => {
                    token_span = Some(grammar.span);
                    let span = grammar.span;
                    let input_str = r::TypeRepr::Ref {
                        lifetime: Some(Lifetime::input()),
                        mutable: false,
                        referent: Box::new(r::TypeRepr::Nominal(r::NominalTypeRepr {
                            path: r::Path::str(),
                            types: vec![],
                        })),
                    };
                    self.conversions
                        .extend(data.match_entries.iter().enumerate().filter_map(
                            |(index, match_entry)| match &match_entry.user_name {
                                MatchMapping::Terminal(user_name) => {
                                    let pattern = Pattern {
                                        span,
                                        kind: PatternKind::TupleStruct(
                                            internal_token_path.clone(),
                                            vec![
                                                Pattern {
                                                    span,
                                                    kind: PatternKind::Usize(index),
                                                },
                                                Pattern {
                                                    span,
                                                    kind: PatternKind::Choose(input_str.clone()),
                                                },
                                            ],
                                        ),
                                    };

                                    Some((user_name.clone(), pattern))
                                }
                                MatchMapping::Skip => None,
                            },
                        ));
                    self.intern_token = Some(data);
                }

                pt::GrammarItem::ExternToken(data) => {
                    if let Some(enum_token) = data.enum_token {
                        token_span = Some(enum_token.type_span);
                        self.conversions
                            .extend(enum_token.conversions.iter().map(|conversion| {
                                (
                                    conversion.from.clone(),
                                    conversion.to.map(&mut |t| t.type_repr()),
                                )
                            }));
                    }
                }

                pt::GrammarItem::Nonterminal(nt) => {
                    let nt_name = &nt.name;
                    let productions: Vec<_> = nt
                        .alternatives
                        .into_iter()
                        .map(|alt| {
                            let nt_type = self.types.nonterminal_type(nt_name).clone();
                            let symbols = self.symbols(&alt.expr.symbols);
                            let action = self.action_kind(nt_type, &alt.expr, &symbols, alt.action);
                            r::Production {
                                nonterminal: nt_name.clone(),
                                span: alt.span,
                                symbols,
                                action,
                            }
                        })
                        .collect();
                    self.nonterminals.insert(
                        nt_name.clone(),
                        r::NonterminalData {
                            name: nt_name.clone(),
                            visibility: nt.visibility.clone(),
                            annotations: nt.annotations,
                            span: nt.span,
                            productions,
                        },
                    );
                }
            }
        }

        let parameters = grammar
            .parameters
            .iter()
            .map(|p| r::Parameter {
                name: p.name.clone(),
                ty: p.ty.type_repr(),
            })
            .collect();

        let where_clauses = grammar
            .where_clauses
            .iter()
            .flat_map(|wc| self.lower_where_clause(wc))
            .collect();

        let mut algorithm = r::Algorithm::default();

        // FIXME Error recovery only works for parse tables so temporarily only generate parse tables for
        // testing
        if self.session.unit_test && !self.uses_error_recovery {
            algorithm.codegen = r::LrCodeGeneration::TestAll;
        }

        read_algorithm(&grammar.annotations, &mut algorithm);

        let mut all_terminals: Vec<_> = self
            .conversions
            .iter()
            .map(|c| c.0.clone())
            .chain(if self.uses_error_recovery {
                Some(TerminalString::Error)
            } else {
                None
            })
            .collect();
        all_terminals.sort();

        let terminal_bits: Map<_, _> = all_terminals.iter().cloned().zip(0..).collect();

        Ok(r::Grammar {
            uses_error_recovery: self.uses_error_recovery,
            prefix: self.prefix,
            start_nonterminals: start_symbols,
            uses,
            action_fn_defns: self.action_fn_defns,
            nonterminals: self.nonterminals,
            conversions: self.conversions.into_iter().collect(),
            types: self.types,
            token_span: token_span.unwrap(),
            type_parameters: grammar.type_parameters,
            parameters,
            where_clauses,
            algorithm,
            intern_token: self.intern_token,
            terminals: r::TerminalSet {
                all: all_terminals,
                bits: terminal_bits,
            },
            module_attributes: grammar.module_attributes,
        })
    }

    fn synthesize_start_symbols(
        &mut self,
        grammar: &pt::Grammar,
    ) -> Map<NonterminalString, NonterminalString> {
        let session = self.session;
        grammar
            .items
            .iter()
            .filter_map(GrammarItem::as_nonterminal)
            .filter(|nt| nt.visibility.is_pub())
            .filter(|nt| cfg_active(session, nt))
            .map(|nt| {
                // create a synthetic symbol `__Foo` for each public symbol `Foo`
                // with a rule like:
                //
                //     __Foo = Foo;
                let fake_name =
                    pt::NonterminalString(Atom::from(format!("{}{}", self.prefix, nt.name)));
                let nt_type = self.types.nonterminal_type(&nt.name).clone();
                self.types.add_type(fake_name.clone(), nt_type.clone());
                let expr = pt::ExprSymbol {
                    symbols: vec![pt::Symbol::new(
                        nt.span,
                        pt::SymbolKind::Nonterminal(fake_name.clone()),
                    )],
                };
                let symbols = vec![r::Symbol::Nonterminal(nt.name.clone())];
                let action_fn = self.action_fn(nt_type, false, &expr, &symbols, None);
                let production = r::Production {
                    nonterminal: fake_name.clone(),
                    symbols,
                    action: action_fn,
                    span: nt.span,
                };
                self.nonterminals.insert(
                    fake_name.clone(),
                    r::NonterminalData {
                        name: fake_name.clone(),
                        visibility: nt.visibility.clone(),
                        annotations: vec![],
                        span: nt.span,
                        productions: vec![production],
                    },
                );
                (nt.name.clone(), fake_name)
            })
            .collect()
    }

    /// When we lower where clauses into `repr::WhereClause`, they get
    /// flattened; so we may go from `T: Foo + Bar` into `[T: Foo, T:
    /// Bar]`. We also convert to `TypeRepr` and so forth.
    fn lower_where_clause(&mut self, wc: &pt::WhereClause<pt::TypeRef>) -> Vec<r::WhereClause> {
        match wc {
            pt::WhereClause::Lifetime { lifetime, bounds } => bounds
                .iter()
                .map(|bound| r::WhereClause::Bound {
                    subject: r::TypeRepr::Lifetime(lifetime.clone()),
                    bound: pt::TypeBound::Lifetime(bound.clone()),
                })
                .collect(),

            pt::WhereClause::Type { forall, ty, bounds } => bounds
                .iter()
                .map(|bound| r::WhereClause::Bound {
                    subject: ty.type_repr(),
                    bound: bound.map(pt::TypeRef::type_repr),
                })
                .map(|bound| {
                    if forall.is_empty() {
                        bound
                    } else {
                        r::WhereClause::Forall {
                            binder: forall.clone(),
                            clause: Box::new(bound),
                        }
                    }
                })
                .collect(),
        }
    }

    fn action_kind(
        &mut self,
        nt_type: r::TypeRepr,
        expr: &pt::ExprSymbol,
        symbols: &[r::Symbol],
        action: Option<pt::ActionKind>,
    ) -> r::ActionFn {
        match action {
            Some(pt::ActionKind::Lookahead) => self.lookahead_action_fn(),
            Some(pt::ActionKind::Lookbehind) => self.lookbehind_action_fn(),
            Some(pt::ActionKind::User(string)) => {
                self.action_fn(nt_type, false, expr, symbols, Some(string))
            }
            Some(pt::ActionKind::Fallible(string)) => {
                self.action_fn(nt_type, true, expr, symbols, Some(string))
            }
            None => self.action_fn(nt_type, false, expr, symbols, None),
        }
    }

    fn lookahead_action_fn(&mut self) -> r::ActionFn {
        let action_fn_defn = r::ActionFnDefn {
            fallible: false,
            ret_type: self.types.terminal_loc_type(),
            kind: r::ActionFnDefnKind::Lookaround(r::LookaroundActionFnDefn::Lookahead),
        };

        self.add_action_fn(action_fn_defn)
    }

    fn lookbehind_action_fn(&mut self) -> r::ActionFn {
        let action_fn_defn = r::ActionFnDefn {
            fallible: false,
            ret_type: self.types.terminal_loc_type(),
            kind: r::ActionFnDefnKind::Lookaround(r::LookaroundActionFnDefn::Lookbehind),
        };

        self.add_action_fn(action_fn_defn)
    }

    fn action_fn(
        &mut self,
        nt_type: r::TypeRepr,
        fallible: bool,
        expr: &pt::ExprSymbol,
        symbols: &[r::Symbol],
        action: Option<String>,
    ) -> r::ActionFn {
        let normalized_symbols = norm_util::analyze_expr(expr);

        let action = match action {
            Some(s) => s,
            None => {
                // If the user declared a type `()`, or we inferred
                // it, then there is only one possible action that
                // will type-check (`()`), so supply that. Otherwise,
                // default is to include all selected items.
                if nt_type.is_unit() {
                    "()".to_string()
                } else {
                    let len = match &normalized_symbols {
                        Symbols::Named(names) => names.len(),
                        Symbols::Anon(indices) => indices.len(),
                    };
                    if len == 1 {
                        "<>".to_string()
                    } else {
                        "(<>)".to_string()
                    }
                }
            }
        };

        // Note that the action fn takes ALL of the symbols in `expr`
        // as arguments, and some of them are simply dropped based on
        // the user's selections.

        // The set of argument types is thus the type of all symbols:
        let arg_types: Vec<r::TypeRepr> =
            symbols.iter().map(|s| s.ty(&self.types)).cloned().collect();

        let action_fn_defn = match normalized_symbols {
            Symbols::Named(names) => {
                // if there are named symbols, we want to give the
                // arguments the names that the user gave them:
                let arg_names = names.iter().map(|(index, name, _)| (*index, name.clone()));
                let arg_patterns = patterns(arg_names, symbols.len());

                let action = {
                    match norm_util::check_between_braces(&action) {
                        norm_util::Presence::None => action,
                        norm_util::Presence::Normal => {
                            let name_str: String = {
                                let name_strs: Vec<_> = names
                                    .iter()
                                    .map(|&(_, ref name, _)| name.name.as_ref())
                                    .collect();
                                name_strs.join(", ")
                            };
                            action.replace("<>", &name_str)
                        }
                        norm_util::Presence::InCurlyBrackets => {
                            let name_str = {
                                let name_strs: Vec<_> = names
                                    .iter()
                                    .map(|&(_, ref name, _)| format!("{0}:{0}", &*name.name))
                                    .collect();
                                name_strs.join(", ")
                            };
                            action.replace("<>", &name_str)
                        }
                    }
                };

                r::ActionFnDefn {
                    fallible,
                    ret_type: nt_type,
                    kind: r::ActionFnDefnKind::User(r::UserActionFnDefn {
                        arg_patterns,
                        arg_types,
                        code: action,
                    }),
                }
            }
            Symbols::Anon(indices) => {
                let names: Vec<_> = (0..indices.len()).map(|i| self.fresh_name(i)).collect();

                let p_indices = indices.iter().map(|&(index, _)| index);
                let p_names = names.iter().cloned().map(Name::immut);
                let arg_patterns = patterns(p_indices.zip(p_names), symbols.len());

                let name_str = {
                    let name_strs: Vec<_> = names.iter().map(AsRef::as_ref).collect();
                    name_strs.join(", ")
                };
                let action = action.replace("<>", &name_str);
                r::ActionFnDefn {
                    fallible,
                    ret_type: nt_type,
                    kind: r::ActionFnDefnKind::User(r::UserActionFnDefn {
                        arg_patterns,
                        arg_types,
                        code: action,
                    }),
                }
            }
        };

        self.add_action_fn(action_fn_defn)
    }

    fn add_action_fn(&mut self, action_fn_defn: r::ActionFnDefn) -> r::ActionFn {
        let index = r::ActionFn::new(self.action_fn_defns.len());
        self.action_fn_defns.push(action_fn_defn);
        index
    }

    fn symbols(&mut self, symbols: &[pt::Symbol]) -> Vec<r::Symbol> {
        symbols.iter().map(|sym| self.symbol(sym)).collect()
    }

    fn symbol(&mut self, symbol: &pt::Symbol) -> r::Symbol {
        match symbol.kind {
            pt::SymbolKind::Terminal(ref id) => r::Symbol::Terminal(id.clone()),
            pt::SymbolKind::Nonterminal(ref id) => r::Symbol::Nonterminal(id.clone()),
            pt::SymbolKind::Choose(ref s) | pt::SymbolKind::Name(_, ref s) => self.symbol(s),
            pt::SymbolKind::Error => {
                self.uses_error_recovery = true;
                r::Symbol::Terminal(TerminalString::Error)
            }

            pt::SymbolKind::Macro(..)
            | pt::SymbolKind::Repeat(..)
            | pt::SymbolKind::Expr(..)
            | pt::SymbolKind::AmbiguousId(_)
            | pt::SymbolKind::Lookahead
            | pt::SymbolKind::Lookbehind => unreachable!(
                "symbol `{}` should have been normalized away by now",
                symbol
            ),
        }
    }

    fn fresh_name(&self, i: usize) -> Atom {
        Atom::from(format!("{}{}", self.prefix, i))
    }
}

fn patterns<I>(mut chosen: I, num_args: usize) -> Vec<Name>
where
    I: Iterator<Item = (usize, Name)>,
{
    let blank = Atom::from("_");

    let mut next_chosen = chosen.next();

    let result = (0..num_args)
        .map(|index| match next_chosen.clone() {
            Some((chosen_index, ref chosen_name)) if chosen_index == index => {
                next_chosen = chosen.next();
                chosen_name.clone()
            }
            _ => Name::immut(blank.clone()),
        })
        .collect();

    debug_assert!(next_chosen.is_none());

    result
}

fn cfg_active(session: &Session, nt: &pt::NonterminalData) -> bool {
    let cfg_atom = Atom::from(CFG);
    nt.annotations
        .iter()
        .filter(|ann| ann.id == cfg_atom)
        .all(|ann| {
            ann.arg.as_ref().map_or(false, |&(_, ref feature)| {
                session
                    .features
                    .as_ref()
                    .map_or(false, |features| features.contains(feature))
            })
        })
}
