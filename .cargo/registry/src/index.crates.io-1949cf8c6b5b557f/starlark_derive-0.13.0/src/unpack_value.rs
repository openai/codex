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

use syn::spanned::Spanned;

use crate::starlark_type_repr::StarlarkTypeReprInput;
use crate::v_lifetime::find_v_lifetime;

pub(crate) fn derive_unpack_value(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match derive_unpack_value_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_unpack_value_impl(input: syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let span = input.ident.span();

    let input = StarlarkTypeReprInput::parse(input, "UnpackValue")?;

    let ident = input.ident;
    let lifetime = find_v_lifetime(&input.generics)?;

    let (_impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    let mut generics = input.generics.clone();
    if lifetime.is_none() {
        generics
            .params
            .push(syn::parse_quote_spanned! { span => 'v });
    }

    let (impl_generics, _type_generics, _where_clause) = generics.split_for_impl();

    let branches: Vec<syn::ExprIf> = input
        .enum_variants
        .iter()
        .enumerate()
        .map(|(i, (n, t))| {
            let mut map_err: syn::Expr = syn::parse_quote_spanned! { t.span() => starlark::__macro_refs::Either::Left(e) };
            for _ in 0..i {
                map_err = syn::parse_quote_spanned! { t.span() => starlark::__macro_refs::Either::Right(#map_err) };
            }
            syn::parse_quote_spanned! { t.span() =>
                if let Some(x) = <#t as starlark::values::UnpackValue<'v>>::unpack_value_impl(value).map_err(|e| #map_err)? {
                    return std::result::Result::Ok(Some(#ident::#n(x)));
                }
            }
        })
        .collect::<Vec<_>>();

    // `Either<A::Error, Either<B::Error, Either<C::Error, Infallible>>>`
    let mut error: syn::Type = syn::parse_quote_spanned! { span => std::convert::Infallible };
    for variant in input.enum_variants.iter().rev() {
        let t = &variant.1;
        error = syn::parse_quote_spanned! { variant.1.span() =>
            starlark::__macro_refs::Either<<#t as starlark::values::UnpackValue<'v>>::Error, #error>
        };
    }

    let trait_impl: syn::ItemImpl = syn::parse_quote_spanned! { span =>
        #[allow(clippy::all)]
        impl #impl_generics starlark::values::UnpackValue<'v> for #ident #type_generics #where_clause {
            type Error = #error;

            fn unpack_value_impl(value: starlark::values::Value<'v>) -> std::result::Result<std::option::Option<Self>, Self::Error> {
                #(#branches)*
                let _unused_when_enum_is_empty = value;
                std::result::Result::Ok(None)
            }
        }
    };
    Ok(quote::quote_spanned! { span => #trait_impl })
}
