/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;
use syn::parse_macro_input;
use syn::spanned::Spanned;
use syn::DeriveInput;

use crate::util::DeriveInputUtil;

fn derive_body(input: &DeriveInput) -> syn::Result<syn::Expr> {
    let derive_input = DeriveInputUtil::new(input)?;
    derive_input.for_each_field(|field_name, field| {
        Ok(quote_spanned! {
            field.span() =>
            crate::eval::runtime::visit_span::VisitSpanMut::visit_spans(#field_name, visitor);
        })
    })
}

fn derive(input: DeriveInput) -> syn::Result<TokenStream> {
    let input = DeriveInputUtil::new(&input)?;
    let (_impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.ident;
    let body = derive_body(&input)?;

    let impl_generics = if input.generics.params.is_empty() {
        quote! {}
    } else {
        let params = input
            .generics()
            .assert_only_type_params()?
            .into_iter()
            .map(|t| {
                let t = &t.ident;
                quote! {
                    #t: crate::eval::runtime::visit_span::VisitSpanMut
                }
            })
            .collect::<Vec<_>>();
        quote! {
            < #(#params,)* >
        }
    };

    Ok(quote! {
        impl #impl_generics crate::eval::runtime::visit_span::VisitSpanMut for #name #type_generics #where_clause {
            fn visit_spans(&mut self, visitor: &mut impl FnMut(&mut crate::eval::runtime::frame_span::FrameSpan)) {
                #body
            }
        }
    })
}

pub(crate) fn derive_visit_span_mut(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match derive(input) {
        Ok(output) => output.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
