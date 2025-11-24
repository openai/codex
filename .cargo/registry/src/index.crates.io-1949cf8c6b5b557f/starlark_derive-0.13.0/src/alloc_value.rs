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

use quote::quote;
use syn::spanned::Spanned;
use syn::DeriveInput;
use syn::Fields;

use crate::util::DataEnumUtil;
use crate::util::DeriveInputUtil;
use crate::v_lifetime::find_v_lifetime;

#[derive(Copy, Clone)]
enum WhichTrait {
    AllocValue,
    AllocFrozenValue,
}

pub(crate) fn derive_alloc_value(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    match derive_alloc_value_impl(input, WhichTrait::AllocValue) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

pub(crate) fn derive_alloc_frozen_value(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    match derive_alloc_value_impl(input, WhichTrait::AllocFrozenValue) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_alloc_value_impl(
    derive_input: DeriveInput,
    which_trait: WhichTrait,
) -> syn::Result<proc_macro2::TokenStream> {
    let derive_input = DeriveInputUtil::new(&derive_input)?;
    let DeriveInputUtil::Enum(en) = derive_input else {
        return Err(syn::Error::new(
            derive_input.span(),
            "`AllocValue` can only be derived for enums",
        ));
    };
    let (_impl_generics, type_generics, where_clause) = derive_input.generics.split_for_impl();

    let mut generics = derive_input.generics.clone();
    match which_trait {
        WhichTrait::AllocValue => {
            let lifetime = find_v_lifetime(&derive_input.generics)?;
            if lifetime.is_none() {
                generics.params.push(syn::parse_quote!('v));
            }
        }
        WhichTrait::AllocFrozenValue => {}
    }

    let type_name = &derive_input.ident;

    let body = alloc_value_body(en, which_trait)?;

    let item_impl: syn::ItemImpl = match which_trait {
        WhichTrait::AllocValue => {
            syn::parse_quote_spanned! {
                derive_input.span() =>
                    impl #generics starlark::values::AllocValue<'v> for #type_name #type_generics #where_clause {
                        fn alloc_value(
                            self,
                            heap: &'v starlark::values::Heap,
                        ) -> starlark::values::Value<'v> {
                            let _ignore_heap_for_empty_enums = heap;
                            #body
                        }
                    }
            }
        }
        WhichTrait::AllocFrozenValue => {
            syn::parse_quote_spanned! {
            derive_input.span() =>
                impl #generics starlark::values::AllocFrozenValue for #type_name #type_generics #where_clause {
                    fn alloc_frozen_value(
                        self,
                        heap: &starlark::values::FrozenHeap,
                    ) -> starlark::values::FrozenValue {
                        let _ignore_heap_for_empty_enums = heap;
                        #body
                    }
                }
            }
        }
    };

    Ok(quote! {
        #item_impl
    })
}

fn alloc_value_body(en: DataEnumUtil, which_trait: WhichTrait) -> syn::Result<syn::Expr> {
    en.match_self(|variant, fields| {
        match &variant.fields {
            Fields::Unnamed(_) => {}
            _ => {
                return Err(syn::Error::new(
                    variant.span(),
                    "Enum variant must has single unnamed field",
                ));
            }
        }
        let [(var, _field)] = fields.as_slice() else {
            return Err(syn::Error::new(
                variant.span(),
                "Enum variant must have exactly one field",
            ));
        };
        match which_trait {
            WhichTrait::AllocValue => Ok(syn::parse_quote_spanned! {
                variant.span() =>
                    starlark::values::AllocValue::alloc_value(#var, heap)
            }),
            WhichTrait::AllocFrozenValue => Ok(syn::parse_quote_spanned! {
                variant.span() =>
                    starlark::values::AllocFrozenValue::alloc_frozen_value(#var, heap)
            }),
        }
    })
}
