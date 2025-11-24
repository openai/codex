/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use quote::quote;
use syn::parse_macro_input;
use syn::DeriveInput;

pub fn derive_copy_(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let name = &input.ident;
    let gen = quote! {
        impl #impl_generics ::std::marker::Copy for #name #ty_generics #where_clause {
        }
    };
    gen.into()
}
