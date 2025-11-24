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

use proc_macro2::Span;
use quote::quote;
use syn::parse_macro_input;
use syn::DeriveInput;
use syn::GenericParam;
use syn::Lifetime;
use syn::LifetimeParam;

pub fn derive_no_serialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let tick_v = GenericParam::Lifetime(LifetimeParam::new(Lifetime::new("'v", Span::call_site())));

    let mut has_tick_v = false;
    for param in &input.generics.params {
        if let GenericParam::Lifetime(t) = param {
            if t.lifetime.ident == "v" {
                has_tick_v = true;
            }
        }
    }
    let mut generics2 = input.generics.clone();

    let (_, ty_generics, _) = input.generics.split_for_impl();
    if !has_tick_v {
        generics2.params.insert(0, tick_v);
    }
    let (impl_generics, _, _) = generics2.split_for_impl();

    let name = &input.ident;
    let gen = quote! {
        impl #impl_generics starlark::__derive_refs::serde::Serialize for #name #ty_generics where Self : starlark::values::StarlarkValue<'v> {
            fn serialize<__NoSerializeS>(&self, serializer: __NoSerializeS) -> std::result::Result<__NoSerializeS::Ok, __NoSerializeS::Error>
            where
                __NoSerializeS: starlark::__derive_refs::serde::Serializer,
            {
                Err(starlark::__derive_refs::serde::Error::custom(format!("Operation `serde::serialize` not supported on type `{}`", <Self as starlark::values::StarlarkValue>::TYPE)))
            }
        }
    };

    gen.into()
}
