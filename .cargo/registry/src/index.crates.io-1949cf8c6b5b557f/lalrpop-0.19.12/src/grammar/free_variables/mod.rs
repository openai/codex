use crate::grammar::parse_tree::{self, Lifetime, TypeParameter};
use crate::grammar::repr;
use std::iter;
use string_cache::DefaultAtom as Atom;

mod test;

/// Finds the set of "free variables" in something -- that is, the
/// type/lifetime parameters that appear and are not bound. For
/// example, `T: Foo<U>` would return `[T, U]`.
pub trait FreeVariables {
    fn free_variables(&self, type_parameters: &[TypeParameter]) -> Vec<TypeParameter>;
}

/// Subtle: the free-variables code sometimes encounter ambiguous
/// names.  For example, we might see `Vec<Foo>` -- in that case, we
/// look at the list of declared type parameters to decide whether
/// `Foo` is a type parameter or just some other type name.
fn free_type(type_parameters: &[TypeParameter], id: &Atom) -> Vec<TypeParameter> {
    let tp = TypeParameter::Id(id.clone());
    if type_parameters.contains(&tp) {
        vec![tp]
    } else {
        vec![]
    }
}

/// Same as above: really, the only lifetime where this is relevant is
/// `'static`, but it doesn't hurt to be careful.
fn free_lifetime(type_parameters: &[TypeParameter], lt: &Lifetime) -> Vec<TypeParameter> {
    let tp = TypeParameter::Lifetime(lt.clone());
    if type_parameters.contains(&tp) {
        vec![tp]
    } else {
        vec![]
    }
}

impl<T: FreeVariables> FreeVariables for Option<T> {
    fn free_variables(&self, type_parameters: &[TypeParameter]) -> Vec<TypeParameter> {
        match self {
            None => vec![],
            Some(t) => t.free_variables(type_parameters),
        }
    }
}

impl<T: FreeVariables> FreeVariables for Vec<T> {
    fn free_variables(&self, type_parameters: &[TypeParameter]) -> Vec<TypeParameter> {
        self.iter()
            .flat_map(|e| e.free_variables(type_parameters))
            .collect()
    }
}

impl FreeVariables for repr::TypeRepr {
    fn free_variables(&self, type_parameters: &[TypeParameter]) -> Vec<TypeParameter> {
        match self {
            repr::TypeRepr::Tuple(tys) => tys.free_variables(type_parameters),
            repr::TypeRepr::Slice(ty) => ty.free_variables(type_parameters),
            repr::TypeRepr::Nominal(data) | repr::TypeRepr::TraitObject(data) => {
                data.free_variables(type_parameters)
            }
            repr::TypeRepr::Associated { type_parameter, .. } => {
                free_type(type_parameters, type_parameter)
            }
            repr::TypeRepr::Lifetime(l) => free_lifetime(type_parameters, l),
            repr::TypeRepr::Ref {
                lifetime, referent, ..
            } => lifetime
                .iter()
                .map(|id| TypeParameter::Lifetime(id.clone()))
                .chain(referent.free_variables(type_parameters))
                .collect(),
            repr::TypeRepr::Fn {
                forall,
                path,
                parameters,
                ret,
            } => path
                .free_variables(type_parameters)
                .into_iter()
                .chain(
                    parameters
                        .iter()
                        .flat_map(|param| param.free_variables(type_parameters)),
                )
                .chain(
                    ret.iter()
                        .flat_map(|ret| ret.free_variables(type_parameters)),
                )
                .filter(|tp| !forall.contains(tp))
                .collect(),
        }
    }
}

impl FreeVariables for repr::WhereClause {
    fn free_variables(&self, type_parameters: &[TypeParameter]) -> Vec<TypeParameter> {
        match self {
            repr::WhereClause::Forall { binder, clause } => clause
                .free_variables(type_parameters)
                .into_iter()
                .filter(|tp| !binder.contains(tp))
                .collect(),

            repr::WhereClause::Bound { subject, bound } => subject
                .free_variables(type_parameters)
                .into_iter()
                .chain(bound.free_variables(type_parameters))
                .collect(),
        }
    }
}

impl FreeVariables for parse_tree::Path {
    fn free_variables(&self, type_parameters: &[TypeParameter]) -> Vec<TypeParameter> {
        // A path like `foo::Bar` is considered no free variables; a
        // single identifier like `T` is a free variable `T`. Note
        // that we can't distinguish type parameters from random names
        // like `String`.
        match self.as_id() {
            Some(id) => free_type(type_parameters, &id),
            None => vec![],
        }
    }
}

impl FreeVariables for repr::NominalTypeRepr {
    fn free_variables(&self, type_parameters: &[TypeParameter]) -> Vec<TypeParameter> {
        let repr::NominalTypeRepr { path, types } = self;
        path.free_variables(type_parameters)
            .into_iter()
            .chain(types.free_variables(type_parameters))
            .collect()
    }
}

impl<T: FreeVariables> FreeVariables for parse_tree::WhereClause<T> {
    fn free_variables(&self, type_parameters: &[TypeParameter]) -> Vec<TypeParameter> {
        match self {
            parse_tree::WhereClause::Lifetime { lifetime, bounds } => {
                iter::once(TypeParameter::Lifetime(lifetime.clone()))
                    .chain(bounds.iter().map(|l| TypeParameter::Lifetime(l.clone())))
                    .collect()
            }

            parse_tree::WhereClause::Type { forall, ty, bounds } => ty
                .free_variables(type_parameters)
                .into_iter()
                .chain(bounds.free_variables(type_parameters))
                .filter(|tp| !forall.contains(tp))
                .collect(),
        }
    }
}

impl<T: FreeVariables> FreeVariables for parse_tree::TypeBoundParameter<T> {
    fn free_variables(&self, type_parameters: &[TypeParameter]) -> Vec<TypeParameter> {
        match self {
            parse_tree::TypeBoundParameter::Lifetime(l) => free_lifetime(type_parameters, l),
            parse_tree::TypeBoundParameter::TypeParameter(t) => t.free_variables(type_parameters),
            parse_tree::TypeBoundParameter::Associated(..) => vec![],
        }
    }
}

impl<T: FreeVariables> FreeVariables for parse_tree::TypeBound<T> {
    fn free_variables(&self, type_parameters: &[TypeParameter]) -> Vec<TypeParameter> {
        match self {
            parse_tree::TypeBound::Lifetime(l) => free_lifetime(type_parameters, l),
            parse_tree::TypeBound::Fn {
                forall,
                parameters,
                ret,
                ..
            } => parameters
                .free_variables(type_parameters)
                .into_iter()
                .chain(ret.free_variables(type_parameters))
                .filter(|tp| !forall.contains(tp))
                .collect(),
            parse_tree::TypeBound::Trait {
                forall, parameters, ..
            } => parameters
                .free_variables(type_parameters)
                .into_iter()
                .filter(|tp| !forall.contains(tp))
                .collect(),
        }
    }
}
