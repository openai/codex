//! Implementation of an [`AsMut`] derive macro.

use proc_macro2::TokenStream;
use quote::format_ident;
use syn::Token;

/// Expands an [`AsMut`] derive macro.
pub(crate) fn expand(
    input: &syn::DeriveInput,
    trait_name: &'static str,
) -> syn::Result<TokenStream> {
    let trait_ident = format_ident!("{trait_name}");
    let method_ident = format_ident!("as_mut");
    let mutability = <Token![mut]>::default();

    super::expand(input, (&trait_ident, &method_ident, Some(&mutability)))
}
