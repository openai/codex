use crate::{
    usage::{self, UsesLifetimes, UsesTypeParams},
    FromDeriveInput, FromField, FromGenericParam, FromGenerics, FromMeta, FromTypeParam,
    FromVariant, Result,
};

/// An efficient way of discarding data from a syntax element.
///
/// All syntax elements will be successfully read into
/// the `Ignored` struct, with all properties discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Ignored;

macro_rules! ignored {
    ($trayt:ident, $method:ident, $syn:path) => {
        impl $trayt for Ignored {
            fn $method(_: &$syn) -> Result<Self> {
                Ok(Ignored)
            }
        }
    };
}

ignored!(FromGenericParam, from_generic_param, syn::GenericParam);
ignored!(FromGenerics, from_generics, syn::Generics);
ignored!(FromTypeParam, from_type_param, syn::TypeParam);
ignored!(FromMeta, from_meta, syn::Meta);
ignored!(FromDeriveInput, from_derive_input, syn::DeriveInput);
ignored!(FromField, from_field, syn::Field);
ignored!(FromVariant, from_variant, syn::Variant);

impl UsesTypeParams for Ignored {
    fn uses_type_params<'a>(
        &self,
        _opts: &usage::Options,
        _: &'a usage::IdentSet,
    ) -> usage::IdentRefSet<'a> {
        Default::default()
    }
}

impl UsesLifetimes for Ignored {
    fn uses_lifetimes<'a>(
        &self,
        _opts: &usage::Options,
        _: &'a usage::LifetimeSet,
    ) -> usage::LifetimeRefSet<'a> {
        Default::default()
    }
}
