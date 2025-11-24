use proc_macro2::Span;
use std::ops::{Deref, DerefMut};
use syn::spanned::Spanned;

use crate::{
    FromDeriveInput, FromField, FromGenericParam, FromGenerics, FromMeta, FromTypeParam,
    FromVariant, Result,
};

/// A value and an associated position in source code. The main use case for this is
/// to preserve position information to emit warnings from proc macros. You can use
/// a `SpannedValue<T>` as a field in any struct that implements or derives any of
/// `darling`'s core traits.
///
/// To access the underlying value, use the struct's `Deref` implementation.
///
/// # Defaulting
/// This type is meant to be used in conjunction with attribute-extracted options,
/// but the user may not always explicitly set those options in their source code.
/// In this case, using `Default::default()` will create an instance which points
/// to `Span::call_site()`.
#[derive(Debug, Clone, Copy)]
pub struct SpannedValue<T> {
    value: T,
    span: Span,
}

impl<T> SpannedValue<T> {
    pub fn new(value: T, span: Span) -> Self {
        SpannedValue { value, span }
    }

    /// Get the source code location referenced by this struct.
    pub fn span(&self) -> Span {
        self.span
    }

    /// Apply a mapping function to a reference to the spanned value.
    pub fn map_ref<U>(&self, map_fn: impl FnOnce(&T) -> U) -> SpannedValue<U> {
        SpannedValue::new(map_fn(&self.value), self.span)
    }
}

impl<T: Default> Default for SpannedValue<T> {
    fn default() -> Self {
        SpannedValue::new(Default::default(), Span::call_site())
    }
}

impl<T> Deref for SpannedValue<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for SpannedValue<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T> AsRef<T> for SpannedValue<T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}

macro_rules! spanned {
    ($trayt:ident, $method:ident, $syn:path) => {
        impl<T: $trayt> $trayt for SpannedValue<T> {
            fn $method(value: &$syn) -> Result<Self> {
                Ok(SpannedValue::new(
                    $trayt::$method(value).map_err(|e| e.with_span(value))?,
                    value.span(),
                ))
            }
        }
    };
}

impl<T: FromMeta> FromMeta for SpannedValue<T> {
    fn from_meta(item: &syn::Meta) -> Result<Self> {
        let value = T::from_meta(item).map_err(|e| e.with_span(item))?;
        let span = match item {
            // Example: `#[darling(skip)]` as SpannedValue<bool>
            // should have the span pointing to the word `skip`.
            syn::Meta::Path(path) => path.span(),
            // Example: `#[darling(attributes(Value))]` as a SpannedValue<Vec<String>>
            // should have the span pointing to the list contents.
            syn::Meta::List(list) => list.tokens.span(),
            // Example: `#[darling(skip = true)]` as SpannedValue<bool>
            // should have the span pointing to the word `true`.
            syn::Meta::NameValue(nv) => nv.value.span(),
        };

        Ok(Self::new(value, span))
    }

    fn from_nested_meta(item: &crate::ast::NestedMeta) -> Result<Self> {
        T::from_nested_meta(item)
            .map(|value| Self::new(value, item.span()))
            .map_err(|e| e.with_span(item))
    }

    fn from_value(literal: &syn::Lit) -> Result<Self> {
        T::from_value(literal)
            .map(|value| Self::new(value, literal.span()))
            .map_err(|e| e.with_span(literal))
    }

    fn from_expr(expr: &syn::Expr) -> Result<Self> {
        T::from_expr(expr)
            .map(|value| Self::new(value, expr.span()))
            .map_err(|e| e.with_span(expr))
    }
}

spanned!(FromGenericParam, from_generic_param, syn::GenericParam);
spanned!(FromGenerics, from_generics, syn::Generics);
spanned!(FromTypeParam, from_type_param, syn::TypeParam);
spanned!(FromDeriveInput, from_derive_input, syn::DeriveInput);
spanned!(FromField, from_field, syn::Field);
spanned!(FromVariant, from_variant, syn::Variant);

impl<T: Spanned> From<T> for SpannedValue<T> {
    fn from(value: T) -> Self {
        let span = value.span();
        SpannedValue::new(value, span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;

    /// Make sure that `SpannedValue` can be seamlessly used as its underlying type.
    #[test]
    fn deref() {
        let test = SpannedValue::new("hello", Span::call_site());
        assert_eq!("hello", test.trim());
    }
}
