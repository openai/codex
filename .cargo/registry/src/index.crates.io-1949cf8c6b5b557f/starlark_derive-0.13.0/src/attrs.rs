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

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::Attribute;
use syn::Data;
use syn::DeriveInput;
use syn::Error;
use syn::Result;
use syn::Token;
use syn::Type;

pub fn derive_attrs(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let data = input.data;
    let name = input.ident;
    expand_attrs_derive(data, name)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

struct Field {
    ident: Ident,
    starlark_args: Vec<Ident>,
    ty: Type,
}

impl Field {
    fn name(&self) -> String {
        let value = self.ident.to_string();
        value.strip_prefix("r#").unwrap_or(&value).to_owned()
    }

    /// Owned String fields should not be cloned before alloc-ing on the
    /// Starlark heap. All other field types are just cloned for convenience of
    /// implementation.
    fn is_owned_string(&self) -> bool {
        match &self.ty {
            Type::Path(p) => {
                p.path == syn::parse_str("std::string::String").unwrap()
                    || p.path.is_ident("String")
            }
            _ => false,
        }
    }

    fn should_clone(&self) -> bool {
        !self.is_owned_string()
    }

    fn skip(&self) -> bool {
        self.starlark_args.iter().any(|i| i == "skip")
    }

    fn has_attr_match_item(&self) -> proc_macro2::TokenStream {
        let name = self.name();
        quote! {
            #name => true
        }
    }

    fn get_attr_match_item(&self) -> proc_macro2::TokenStream {
        let name = self.name();
        let ident = &self.ident;
        match self.should_clone() {
            false => quote! {
                #name => Some(heap.alloc(&self.#ident))
            },
            true => quote! {
                #name => Some(heap.alloc(self.#ident.clone()))
            },
        }
    }
}

fn expand_attrs_derive(data: Data, name: Ident) -> Result<proc_macro2::TokenStream> {
    let fields: Vec<_> = match data {
        Data::Struct(s) => Ok(s.fields.iter().cloned().collect()),
        Data::Enum(e) => Err(Error::new(
            e.enum_token.span(),
            "#[derive(StarlarkAttrs)] does not support enums",
        )),
        Data::Union(u) => Err(Error::new(
            u.union_token.span(),
            "#[derive(StarlarkAttrs)] does not support unions",
        )),
    }?;

    let expose_fields: Vec<Field> = fields
        .into_iter()
        .map(|field| {
            match field_attr(&field, "starlark") {
                // by default, include all fields
                None => Ok(Field {
                    ident: field.ident.unwrap(),
                    starlark_args: vec![],
                    ty: field.ty,
                }),
                Some(attr) => {
                    let starlark_args = attr
                        .parse_args_with(Punctuated::<Ident, Token![,]>::parse_terminated)?
                        .into_iter()
                        .collect();
                    Ok(Field {
                        ident: field.ident.unwrap(),
                        starlark_args,
                        ty: field.ty,
                    })
                }
            }
        })
        .filter(|f| f.as_ref().map(|f| !f.skip()).unwrap_or(true))
        .collect::<Result<_>>()?;

    let has_attr_items = expose_fields.iter().map(|f| f.has_attr_match_item());
    let has_attr = quote! {
        pub(crate) fn attrs_has_attr(&self, attr: &str) -> bool {
            match attr {
                #(#has_attr_items),*,
                _ => false,
            }
        }
    };

    let get_attr_items = expose_fields.iter().map(|f| f.get_attr_match_item());
    let get_attr = quote! {
        pub(crate) fn attrs_get_attr<'v>(&self, attr: &str, heap: &'v starlark::values::Heap) -> Option<starlark::values::Value<'v>> {
            match attr {
                #(#get_attr_items),*,
                _ => None,
            }
        }
    };

    let dir_names = expose_fields.iter().map(|f| f.name());
    let dir_attr = quote! {
        pub(crate) fn attrs_dir_attr(&self) -> Vec<String> {
            vec![
                #(#dir_names.to_owned()),*
            ]
        }
    };

    let expanded = quote! {
        // Unfortunately, we can't actually implement the direct methods for
        // `StarlarkValue`, because then we would have conflicting
        // implementations. However, we can implement wrappers in another
        // proc-macro that can then be called.
        impl #name {
            #has_attr
            #get_attr
            #dir_attr
        }
    };

    Ok(expanded)
}

fn field_attr<'a, I: ?Sized>(field: &'a syn::Field, path: &I) -> Option<&'a Attribute>
where
    Ident: PartialEq<I>,
{
    field.attrs.iter().find(|a| a.path().is_ident(path))
}

pub fn starlark_attrs() -> TokenStream {
    // Clearly, this could be a regular macro, but since it is required by the
    // derive macro, export it as a proc-macro anyway so that it can come from
    // the same crate.
    let expanded = quote! {
        // proxy all attr methods to the implementations generated by
        // derive(Attrs)
        fn has_attr(&self, attr: &str, _heap: &'v starlark::values::Heap) -> bool {
            self.attrs_has_attr(attr)
        }
        fn get_attr(&self, attr: &str, heap: &'v starlark::values::Heap) -> Option<starlark::values::Value<'v>> {
            self.attrs_get_attr(attr, heap)
        }
        fn dir_attr(&self) -> Vec<String> {
            self.attrs_dir_attr()
        }
    };
    expanded.into()
}
