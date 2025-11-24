/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use proc_macro::TokenStream;
use quote::quote_spanned;
use syn::spanned::Spanned;

pub(crate) fn root(_attr: TokenStream, input: TokenStream) -> TokenStream {
    match global_root_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn global_root_impl(input: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    let item_static = syn::parse2::<syn::ItemStatic>(input)?;

    let name = &item_static.ident;

    let register_name = syn::Ident::new(
        &format!("_{}_register_root", name.to_string().to_lowercase()),
        name.span(),
    );

    Ok(quote_spanned! {item_static.span()=>
        #item_static

        #[allocative::__macro_refs::ctor::ctor]
        fn #register_name() {
            allocative::register_root(&#name);
        }
    })
}
