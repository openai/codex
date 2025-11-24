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

use crate::util::duplicate_impl;

pub fn derive_clone_(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let name = &input.ident;
    let body = duplicate_impl(&input.data, &quote! { ::std::clone::Clone::clone });
    let gen = quote! {
        // Clippy wants us to use Copy if we can - we prefer to be agnostic.
        // Add unknown_lints temporarily.
        #[allow(unknown_lints)]
        #[allow(clippy::incorrect_clone_impl_on_copy_type)]
        impl #impl_generics ::std::clone::Clone for #name #ty_generics #where_clause {
            fn clone(&self) -> Self {
                #body
            }
        }
    };
    gen.into()
}
