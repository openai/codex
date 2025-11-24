// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Proc macros for generating `ULE`, `VarULE` impls and types for the `zerovec` crate

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Ident};
mod make_ule;
mod make_varule;
pub(crate) mod ule;
mod utils;
mod varule;

/// Full docs for this proc macro can be found on the [`zerovec`](docs.rs/zerovec) crate.
#[proc_macro_derive(ULE)]
pub fn ule_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(ule::derive_impl(&input))
}

/// Full docs for this proc macro can be found on the [`zerovec`](docs.rs/zerovec) crate.
#[proc_macro_derive(VarULE)]
pub fn varule_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(varule::derive_impl(&input, None))
}

/// Full docs for this proc macro can be found on the [`zerovec`](docs.rs/zerovec) crate.
#[proc_macro_attribute]
pub fn make_ule(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let attr = parse_macro_input!(attr as Ident);
    TokenStream::from(make_ule::make_ule_impl(attr, input))
}

/// Full docs for this proc macro can be found on the [`zerovec`](docs.rs/zerovec) crate.
#[proc_macro_attribute]
pub fn make_varule(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let attr = parse_macro_input!(attr as Ident);
    TokenStream::from(make_varule::make_varule_impl(attr, input))
}
