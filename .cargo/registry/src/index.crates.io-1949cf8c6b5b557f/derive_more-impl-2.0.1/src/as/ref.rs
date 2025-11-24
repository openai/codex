//! Implementation of an [`AsRef`] derive macro.

use proc_macro2::TokenStream;
use quote::format_ident;

/// Expands an [`AsRef`] derive macro.
pub(crate) fn expand(
    input: &syn::DeriveInput,
    trait_name: &'static str,
) -> syn::Result<TokenStream> {
    let trait_ident = format_ident!("{trait_name}");
    let method_ident = format_ident!("as_ref");

    super::expand(input, (&trait_ident, &method_ident, None))
}
