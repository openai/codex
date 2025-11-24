use proc_macro2::TokenStream;
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{GenericParam, Generics, Path, TraitBound, TraitBoundModifier, TypeParamBound};

use crate::codegen::TraitImpl;
use crate::usage::IdentSet;

/// Wrapper for "outer From" traits, such as `FromDeriveInput`, `FromVariant`, and `FromField`.
pub trait OuterFromImpl<'a> {
    /// Gets the path of the trait being implemented.
    fn trait_path(&self) -> Path;

    fn base(&'a self) -> &'a TraitImpl<'a>;

    fn trait_bound(&self) -> Path {
        self.trait_path()
    }

    fn wrap<T: ToTokens>(&'a self, body: T, tokens: &mut TokenStream) {
        let base = self.base();
        let trayt = self.trait_path();
        let ty_ident = base.ident;
        // The type parameters used in non-skipped, non-magic fields.
        // These must impl `FromMeta` unless they have custom bounds.
        let used = base.used_type_params();
        let generics = compute_impl_bounds(self.trait_bound(), base.generics.clone(), &used);
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        tokens.append_all(quote!(
            #[automatically_derived]
            #[allow(clippy::manual_unwrap_or_default)]
            impl #impl_generics #trayt for #ty_ident #ty_generics
                #where_clause
            {
                #body
            }
        ));
    }
}

fn compute_impl_bounds(bound: Path, mut generics: Generics, applies_to: &IdentSet) -> Generics {
    if generics.params.is_empty() {
        return generics;
    }

    let added_bound = TypeParamBound::Trait(TraitBound {
        paren_token: None,
        modifier: TraitBoundModifier::None,
        lifetimes: None,
        path: bound,
    });

    for param in generics.params.iter_mut() {
        if let GenericParam::Type(ref mut typ) = *param {
            if applies_to.contains(&typ.ident) {
                typ.bounds.push(added_bound.clone());
            }
        }
    }

    generics
}
