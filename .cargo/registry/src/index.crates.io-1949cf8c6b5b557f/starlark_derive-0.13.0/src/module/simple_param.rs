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
use quote::ToTokens;
use quote::TokenStreamExt;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::spanned::Spanned;

/// Simple function parameter, subset of what Rust language supports.
#[derive(Debug, Clone)]
pub(crate) struct SimpleParam {
    pub(crate) attrs: Vec<syn::Attribute>,
    pub(crate) mutability: Option<syn::Token![mut]>,
    pub(crate) ident: syn::Ident,
    pub(crate) ty: syn::Type,
}

impl SimpleParam {
    fn fn_arg(&self) -> syn::FnArg {
        let attrs = &self.attrs;
        let mutability = &self.mutability;
        let ident = &self.ident;
        let ty = &self.ty;
        syn::parse_quote! {
            #(#attrs)*
            #mutability #ident: #ty
        }
    }

    pub(crate) fn from_fn_arg(arg: syn::FnArg) -> syn::Result<Self> {
        match arg {
            syn::FnArg::Receiver(_) => Err(syn::Error::new(
                arg.span(),
                "`self` is not used in Starlark native functions",
            )),
            syn::FnArg::Typed(syn::PatType {
                attrs,
                pat,
                colon_token: _,
                ty,
            }) => {
                let syn::Pat::Ident(ident) = *pat else {
                    return Err(syn::Error::new_spanned(
                        pat,
                        "Only simple identifiers are allowed in Starlark function parameters",
                    ));
                };
                let syn::PatIdent {
                    attrs: pat_ident_attrs,
                    by_ref,
                    mutability,
                    ident,
                    subpat,
                } = ident;
                if by_ref.is_some() {
                    return Err(syn::Error::new_spanned(
                        by_ref,
                        "Starlark function parameters cannot use `ref`",
                    ));
                }
                if let Some(subpat) = subpat {
                    return Err(syn::Error::new_spanned(
                        subpat.0,
                        "Starlark function parameters cannot use subpatterns",
                    ));
                }
                if let Some(attr) = pat_ident_attrs.into_iter().next() {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "Starlark function parameters cannot have attributes",
                    ));
                }
                Ok(SimpleParam {
                    attrs,
                    mutability,
                    ident,
                    ty: *ty,
                })
            }
        }
    }
}

impl Parse for SimpleParam {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let arg = syn::FnArg::parse(input)?;
        SimpleParam::from_fn_arg(arg)
    }
}

impl ToTokens for SimpleParam {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.append_all(self.fn_arg().into_token_stream())
    }
}
