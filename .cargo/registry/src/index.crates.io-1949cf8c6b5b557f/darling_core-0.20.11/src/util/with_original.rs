use crate::{
    FromDeriveInput, FromField, FromGenericParam, FromGenerics, FromMeta, FromTypeParam,
    FromVariant, Result,
};

/// A container to parse some syntax and retain access to the original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithOriginal<T, O> {
    pub parsed: T,
    pub original: O,
}

impl<T, O> WithOriginal<T, O> {
    pub fn new(parsed: T, original: O) -> Self {
        WithOriginal { parsed, original }
    }
}

macro_rules! with_original {
    ($trayt:ident, $func:ident, $syn:path) => {
        impl<T: $trayt> $trayt for WithOriginal<T, $syn> {
            fn $func(value: &$syn) -> Result<Self> {
                Ok(WithOriginal::new($trayt::$func(value)?, value.clone()))
            }
        }
    };
}

with_original!(FromDeriveInput, from_derive_input, syn::DeriveInput);
with_original!(FromField, from_field, syn::Field);
with_original!(FromGenerics, from_generics, syn::Generics);
with_original!(FromGenericParam, from_generic_param, syn::GenericParam);
with_original!(FromMeta, from_meta, syn::Meta);
with_original!(FromTypeParam, from_type_param, syn::TypeParam);
with_original!(FromVariant, from_variant, syn::Variant);
