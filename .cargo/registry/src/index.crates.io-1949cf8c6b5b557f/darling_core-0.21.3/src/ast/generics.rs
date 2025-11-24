//! Types for working with generics

use std::iter::Iterator;
use std::slice::Iter;

use crate::{FromGenericParam, FromGenerics, FromTypeParam, Result};

/// Extension trait for `GenericParam` to support getting values by variant.
///
/// # Usage
/// `darling::ast::Generics` needs a way to test its params array in order to iterate over type params.
/// Rather than require callers to use `darling::ast::GenericParam` in all cases, this trait makes that
/// polymorphic.
pub trait GenericParamExt {
    /// The type this GenericParam uses to represent type params and their bounds
    type TypeParam;
    type LifetimeParam;
    type ConstParam;

    /// If this GenericParam is a type param, get the underlying value.
    fn as_type_param(&self) -> Option<&Self::TypeParam> {
        None
    }

    /// If this GenericParam is a lifetime, get the underlying value.
    fn as_lifetime_param(&self) -> Option<&Self::LifetimeParam> {
        None
    }

    /// If this GenericParam is a const param, get the underlying value.
    fn as_const_param(&self) -> Option<&Self::ConstParam> {
        None
    }
}

impl GenericParamExt for syn::GenericParam {
    type TypeParam = syn::TypeParam;
    type LifetimeParam = syn::LifetimeParam;
    type ConstParam = syn::ConstParam;

    fn as_type_param(&self) -> Option<&Self::TypeParam> {
        if let syn::GenericParam::Type(ref val) = *self {
            Some(val)
        } else {
            None
        }
    }

    fn as_lifetime_param(&self) -> Option<&Self::LifetimeParam> {
        if let syn::GenericParam::Lifetime(ref val) = *self {
            Some(val)
        } else {
            None
        }
    }

    fn as_const_param(&self) -> Option<&Self::ConstParam> {
        if let syn::GenericParam::Const(ref val) = *self {
            Some(val)
        } else {
            None
        }
    }
}

impl GenericParamExt for syn::TypeParam {
    type TypeParam = syn::TypeParam;
    type LifetimeParam = ();
    type ConstParam = ();

    fn as_type_param(&self) -> Option<&Self::TypeParam> {
        Some(self)
    }
}

/// A mirror of `syn::GenericParam` which is generic over all its contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenericParam<T = syn::TypeParam, L = syn::LifetimeParam, C = syn::ConstParam> {
    Type(T),
    Lifetime(L),
    Const(C),
}

impl<T: FromTypeParam> FromTypeParam for GenericParam<T> {
    fn from_type_param(type_param: &syn::TypeParam) -> Result<Self> {
        Ok(GenericParam::Type(FromTypeParam::from_type_param(
            type_param,
        )?))
    }
}

impl<T: FromTypeParam> FromGenericParam for GenericParam<T> {
    fn from_generic_param(param: &syn::GenericParam) -> Result<Self> {
        Ok(match *param {
            syn::GenericParam::Type(ref ty) => {
                GenericParam::Type(FromTypeParam::from_type_param(ty)?)
            }
            syn::GenericParam::Lifetime(ref val) => GenericParam::Lifetime(val.clone()),
            syn::GenericParam::Const(ref val) => GenericParam::Const(val.clone()),
        })
    }
}

impl<T, L, C> GenericParamExt for GenericParam<T, L, C> {
    type TypeParam = T;
    type LifetimeParam = L;
    type ConstParam = C;

    fn as_type_param(&self) -> Option<&T> {
        if let GenericParam::Type(ref val) = *self {
            Some(val)
        } else {
            None
        }
    }

    fn as_lifetime_param(&self) -> Option<&L> {
        if let GenericParam::Lifetime(ref val) = *self {
            Some(val)
        } else {
            None
        }
    }

    fn as_const_param(&self) -> Option<&C> {
        if let GenericParam::Const(ref val) = *self {
            Some(val)
        } else {
            None
        }
    }
}

/// A mirror of the `syn::Generics` type which can contain arbitrary representations
/// of params and where clauses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generics<P, W = syn::WhereClause> {
    pub params: Vec<P>,
    pub where_clause: Option<W>,
}

impl<P, W> Generics<P, W> {
    pub fn type_params(&self) -> TypeParams<'_, P> {
        TypeParams(self.params.iter())
    }
}

impl<P: FromGenericParam> FromGenerics for Generics<P> {
    fn from_generics(generics: &syn::Generics) -> Result<Self> {
        Ok(Generics {
            params: generics
                .params
                .iter()
                .map(FromGenericParam::from_generic_param)
                .collect::<Result<Vec<P>>>()?,
            where_clause: generics.where_clause.clone(),
        })
    }
}

pub struct TypeParams<'a, P>(Iter<'a, P>);

impl<'a, P: GenericParamExt> Iterator for TypeParams<'a, P> {
    type Item = &'a <P as GenericParamExt>::TypeParam;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.0.next();
        match next {
            None => None,
            Some(v) => match v.as_type_param() {
                Some(val) => Some(val),
                None => self.next(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::{GenericParam, Generics};
    use crate::FromGenerics;

    #[test]
    fn generics() {
        let g: syn::Generics = parse_quote!(<T>);
        let deified: Generics<GenericParam<syn::Ident>> = FromGenerics::from_generics(&g).unwrap();
        assert!(deified.params.len() == 1);
        assert!(deified.where_clause.is_none());
    }
}
