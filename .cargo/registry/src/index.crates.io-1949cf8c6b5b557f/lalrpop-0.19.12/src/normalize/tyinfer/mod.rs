use super::norm_util::{self, AlternativeAction, Symbols};
use super::{NormError, NormResult};

use crate::grammar::consts::{ERROR, LOCATION};
use crate::grammar::parse_tree::{
    ActionKind, Alternative, Grammar, GrammarItem, Lifetime, MatchMapping, NonterminalData,
    NonterminalString, Path, Span, SymbolKind, TypeParameter, TypeRef,
};
use crate::grammar::repr::{NominalTypeRepr, TypeRepr, Types};
use std::collections::{HashMap, HashSet};
use string_cache::DefaultAtom as Atom;

#[cfg(test)]
mod test;

pub fn infer_types(grammar: &Grammar) -> NormResult<Types> {
    let inferencer = TypeInferencer::new(grammar)?;
    inferencer.infer_types()
}

struct TypeInferencer<'grammar> {
    stack: Vec<NonterminalString>,
    nonterminals: HashMap<NonterminalString, NT<'grammar>>,
    types: Types,
    type_parameters: HashSet<Atom>,
}

#[derive(Copy, Clone)]
struct NT<'grammar> {
    span: Span,
    type_decl: &'grammar Option<TypeRef>,
    alternatives: &'grammar Vec<Alternative>,
}

impl<'grammar> TypeInferencer<'grammar> {
    fn new(grammar: &'grammar Grammar) -> NormResult<TypeInferencer<'grammar>> {
        let types = TypeInferencer::make_types(grammar);

        let nonterminals = grammar
            .items
            .iter()
            .filter_map(GrammarItem::as_nonterminal)
            .map(|data| {
                assert!(!data.is_macro_def()); // normalized away by now
                (data.name.clone(), NT::new(data))
            })
            .collect();

        let type_parameters = grammar
            .type_parameters
            .iter()
            .filter_map(|p| match *p {
                TypeParameter::Lifetime(_) => None,
                TypeParameter::Id(ref ty) => Some(ty.clone()),
            })
            .collect();

        Ok(TypeInferencer {
            stack: vec![],
            nonterminals,
            types,
            type_parameters,
        })
    }

    fn make_types(grammar: &Grammar) -> Types {
        let opt_extern_token = grammar.extern_token();

        // Determine error type (if any).
        let error_type = opt_extern_token.and_then(|extern_token| {
            extern_token
                .associated_type(Atom::from(ERROR))
                .map(|tr| tr.type_ref.type_repr())
        });

        // Determine location type and enum type. If using an internal
        // token, that's specified by us, not user.
        if let Some(intern_token) = grammar.intern_token() {
            let loc_type = // usize
                TypeRepr::usize();
            let input_str = // &'input str
                TypeRepr::Ref {
                    lifetime: Some(Lifetime::input()),
                    mutable: false,
                    referent: Box::new(TypeRepr::str())
                };
            let enum_type = // Token<'input>
                TypeRepr::Nominal(NominalTypeRepr {
                    path: Path {
                        absolute: false,
                        ids: vec![Atom::from("Token")],
                    },
                    types: vec![TypeRepr::Lifetime(Lifetime::input())]
                });

            let mut types = Types::new(&grammar.prefix, Some(loc_type), error_type, enum_type);

            for match_entry in &intern_token.match_entries {
                if let MatchMapping::Terminal(user_name) = &match_entry.user_name {
                    types.add_term_type(user_name.clone(), input_str.clone());
                }
            }

            types
        } else {
            let extern_token = opt_extern_token.unwrap();
            let loc_type = extern_token
                .associated_type(Atom::from(LOCATION))
                .map(|tr| tr.type_ref.type_repr());
            let enum_type = extern_token
                .enum_token
                .as_ref()
                .unwrap()
                .type_name
                .type_repr();
            let mut types = Types::new(&grammar.prefix, loc_type, error_type, enum_type);

            // For each defined conversion, figure out the type of the
            // terminal and enter it into `types` by hand if it is not the
            // default. For terminals with custom types, the user should
            // have one or more bindings in the pattern -- if more than
            // one, make a tuple.
            //
            // e.g. "(" => Lparen(..) ==> no custom type
            //      "Num" => Num(<u32>) ==> custom type is u32
            //      "Fraction" => Real(<u32>,<u32>) ==> custom type is (u32, u32)
            for conversion in grammar
                .enum_token()
                .into_iter()
                .flat_map(|et| &et.conversions)
            {
                let mut tys = Vec::new();
                conversion
                    .to
                    .for_each_binding(&mut |ty| tys.push(ty.type_repr()));
                if tys.is_empty() {
                    continue;
                }
                let ty = maybe_tuple(tys);
                types.add_term_type(conversion.from.clone(), ty);
            }

            types
        }
    }

    fn infer_types(mut self) -> NormResult<Types> {
        let ids: Vec<NonterminalString> =
            self.nonterminals.iter().map(|(id, _)| id.clone()).collect();

        for id in ids {
            self.nonterminal_type(&id)?;
            debug_assert!(self.types.lookup_nonterminal_type(&id).is_some());
        }

        Ok(self.types)
    }

    fn nonterminal_type(&mut self, id: &NonterminalString) -> NormResult<TypeRepr> {
        if let Some(repr) = self.types.lookup_nonterminal_type(id) {
            return Ok(repr.clone());
        }

        let nt = self.nonterminals[id];
        if self.stack.contains(id) {
            return_err!(
                nt.span,
                "cannot infer type of `{}` because it references itself",
                id
            );
        }

        let ty = self.push(id, |this| {
            if let Some(ref type_decl) = nt.type_decl {
                return this.type_ref(type_decl);
            }

            // Try to compute the types of all alternatives; note that
            // some may result in an error. Don't report these errors
            // (yet).
            let mut alternative_types = vec![];
            let mut alternative_errors = vec![];
            for alt in nt.alternatives.iter() {
                match this.alternative_type(alt) {
                    Ok(t) => alternative_types.push(t),
                    Err(e) => alternative_errors.push(e),
                }
            }

            // if it never succeeded, report first error
            if alternative_types.is_empty() {
                match alternative_errors.into_iter().next() {
                    Some(err) => {
                        return Err(err);
                    }
                    None => {
                        // if nothing succeeded, and nothing errored,
                        // must have been nothing to start with
                        return_err!(
                            nt.span,
                            "nonterminal `{}` has no alternatives and hence parse cannot succeed",
                            id
                        );
                    }
                }
            }

            // otherwise, check that all the cases where we had success agree
            for ((ty, alt), i) in alternative_types[1..]
                .iter()
                .zip(&nt.alternatives[1..])
                .zip(1..)
            {
                if &alternative_types[0] != ty {
                    return_err!(
                        alt.span,
                        "type of alternative #{} is `{}`, \
                         but type of first alternative is `{}`",
                        i + 1,
                        ty,
                        alternative_types[0]
                    );
                }
            }

            // and use that type
            Ok(alternative_types.pop().unwrap())
        })?;

        self.types.add_type(id.clone(), ty.clone());
        Ok(ty)
    }

    fn push<F, R>(&mut self, id: &NonterminalString, f: F) -> NormResult<R>
    where
        F: FnOnce(&mut TypeInferencer) -> NormResult<R>,
    {
        self.stack.push(id.clone());
        let r = f(self);
        assert_eq!(self.stack.pop().unwrap(), *id);
        r
    }

    fn type_ref(&mut self, type_ref: &TypeRef) -> NormResult<TypeRepr> {
        match *type_ref {
            TypeRef::Tuple(ref types) => {
                let types = types
                    .iter()
                    .map(|t| self.type_ref(t))
                    .collect::<Result<_, _>>()?;
                Ok(TypeRepr::Tuple(types))
            }
            TypeRef::Slice(ref ty) => self.type_ref(ty).map(|ty| TypeRepr::Slice(Box::new(ty))),
            TypeRef::Nominal {
                ref path,
                ref types,
            } => {
                if path.ids.len() == 2 && self.type_parameters.contains(&path.ids[0]) {
                    return Ok(TypeRepr::Associated {
                        type_parameter: path.ids[0].clone(),
                        id: path.ids[1].clone(),
                    });
                }

                let types = types
                    .iter()
                    .map(|t| self.type_ref(t))
                    .collect::<Result<_, _>>()?;
                Ok(TypeRepr::Nominal(NominalTypeRepr {
                    path: path.clone(),
                    types,
                }))
            }
            TypeRef::Lifetime(ref id) => Ok(TypeRepr::Lifetime(id.clone())),
            TypeRef::Id(ref id) => Ok(TypeRepr::Nominal(NominalTypeRepr {
                path: Path::from_id(id.clone()),
                types: vec![],
            })),
            TypeRef::Ref {
                ref lifetime,
                mutable,
                ref referent,
            } => Ok(TypeRepr::Ref {
                lifetime: lifetime.clone(),
                mutable,
                referent: Box::new(self.type_ref(referent)?),
            }),
            TypeRef::OfSymbol(ref symbol) => self.symbol_type(symbol),
            TypeRef::TraitObject {
                ref path,
                ref types,
            } => {
                let types = types
                    .iter()
                    .map(|t| self.type_ref(t))
                    .collect::<Result<_, _>>()?;
                Ok(TypeRepr::TraitObject(NominalTypeRepr {
                    path: path.clone(),
                    types,
                }))
            }
            TypeRef::Fn {
                ref forall,
                ref path,
                ref parameters,
                ref ret,
            } => Ok(TypeRepr::Fn {
                forall: forall.clone(),
                path: path.clone(),
                parameters: parameters
                    .iter()
                    .map(|t| self.type_ref(t))
                    .collect::<Result<_, _>>()?,
                ret: match ret {
                    Some(ret) => Some(self.type_ref(ret).map(Box::new)?),
                    None => None,
                },
            }),
        }
    }

    fn alternative_type(&mut self, alt: &Alternative) -> NormResult<TypeRepr> {
        match norm_util::analyze_action(alt) {
            AlternativeAction::User(&ActionKind::User(_))
            | AlternativeAction::User(&ActionKind::Fallible(_)) => {
                return_err!(
                    alt.span,
                    "cannot infer types if there is custom action code"
                );
            }

            AlternativeAction::User(&ActionKind::Lookahead)
            | AlternativeAction::User(&ActionKind::Lookbehind) => {
                Ok(self.types.opt_terminal_loc_type().unwrap().clone())
            }

            AlternativeAction::Default(Symbols::Named(ref syms)) => {
                return_err!(
                    alt.span,
                    "cannot infer types in the presence of named symbols like `{}:{}`",
                    syms[0].1,
                    syms[0].2
                );
            }

            AlternativeAction::Default(Symbols::Anon(syms)) => {
                let symbol_types: Vec<TypeRepr> = syms
                    .iter()
                    .map(|&(_, sym)| self.symbol_type(&sym.kind))
                    .collect::<Result<_, _>>()?;
                Ok(maybe_tuple(symbol_types))
            }
        }
    }

    fn symbol_type(&mut self, symbol: &SymbolKind) -> NormResult<TypeRepr> {
        match *symbol {
            SymbolKind::Terminal(ref id) => Ok(self.types.terminal_type(id).clone()),
            SymbolKind::Nonterminal(ref id) => self.nonterminal_type(id),
            SymbolKind::Choose(ref s) => self.symbol_type(&s.kind),
            SymbolKind::Name(_, ref s) => self.symbol_type(&s.kind),
            SymbolKind::Error => Ok(self.types.error_recovery_type().clone()),

            SymbolKind::Repeat(..)
            | SymbolKind::Expr(..)
            | SymbolKind::Macro(..)
            | SymbolKind::AmbiguousId(..)
            | SymbolKind::Lookahead
            | SymbolKind::Lookbehind => {
                unreachable!("symbol `{:?}` should have been expanded away", symbol)
            }
        }
    }
}

impl<'grammar> NT<'grammar> {
    fn new(data: &'grammar NonterminalData) -> NT<'grammar> {
        NT {
            span: data.span,
            type_decl: &data.type_decl,
            alternatives: &data.alternatives,
        }
    }
}

fn maybe_tuple(v: Vec<TypeRepr>) -> TypeRepr {
    if v.len() == 1 {
        v.into_iter().next().unwrap()
    } else {
        TypeRepr::Tuple(v)
    }
}
