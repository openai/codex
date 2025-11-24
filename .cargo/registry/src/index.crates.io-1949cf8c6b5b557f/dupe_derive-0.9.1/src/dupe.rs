/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::parse_quote;
use syn::DeriveInput;
use syn::Ident;
use syn::Type;
use syn::TypeParamBound;

use crate::util::add_trait_bounds;
use crate::util::extract_all_field_tys;

pub fn derive_dupe(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_dupe_explicit(input, true)
}

pub fn derive_dupe_(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_dupe_explicit(input, false)
}

fn derive_dupe_explicit(
    input: proc_macro::TokenStream,
    with_traits: bool,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;

    // Add a bound `T: Dupe` to every type parameter T.
    let generics = if with_traits {
        let bound: TypeParamBound = parse_quote!(dupe::Dupe);
        add_trait_bounds(input.generics, &bound)
    } else {
        input.generics
    };
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let all_fields = match extract_all_field_tys(&input.data) {
        Ok(r) => r,
        Err(e) => {
            return e.into_compile_error().into();
        }
    };
    let check_each_field_dupe = check_each_field_dupe(all_fields);

    let check_func_name = Ident::new(
        &format!("__implicit_dupe_check_for_fields_of_{}", name),
        name.span(),
    );

    let gen = quote! {
        impl #impl_generics dupe::Dupe for #name #ty_generics #where_clause {
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #check_func_name #impl_generics (_x: #name #ty_generics) #where_clause {
            #check_each_field_dupe
        }
    };

    gen.into()
}

fn check_each_field_dupe<'a>(tys: impl IntoIterator<Item = &'a Type>) -> TokenStream {
    let tys = tys.into_iter();
    quote! {
        #(
            dupe::__macro_refs::assert_dupe::<#tys>();
        )*
    }
}
