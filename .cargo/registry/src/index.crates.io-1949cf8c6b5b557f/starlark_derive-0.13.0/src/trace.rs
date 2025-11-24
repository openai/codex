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

use std::collections::HashSet;

use proc_macro2::Span;
use quote::quote;
use quote::quote_spanned;
use quote::ToTokens;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::parse_quote;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::Attribute;
use syn::DeriveInput;
use syn::GenericArgument;
use syn::GenericParam;
use syn::Generics;
use syn::Lifetime;
use syn::LifetimeParam;
use syn::PathArguments;
use syn::ReturnType;
use syn::TraitBound;
use syn::Type;
use syn::TypeParamBound;

use crate::util::DeriveInputUtil;

pub(crate) fn derive_trace(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match derive_trace_impl(input) {
        Ok(x) => x.into_token_stream().into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn derive_trace_impl(mut input: DeriveInput) -> syn::Result<syn::ItemImpl> {
    let tick_v = GenericParam::Lifetime(LifetimeParam::new(Lifetime::new("'v", Span::call_site())));

    let TraceAttrs {
        unsafe_ignore,
        trace_static,
        bounds,
    } = parse_attrs(&input.attrs)?;
    if let Some(unsafe_ignore) = unsafe_ignore {
        return Err(syn::Error::new_spanned(
            unsafe_ignore,
            "`unsafe_ignore` attribute is not allowed on `#[derive(Trace)]`, only on fields",
        ));
    }
    if let Some(trace_static) = trace_static {
        return Err(syn::Error::new_spanned(
            trace_static,
            "`static` attribute is not allowed on `#[derive(Trace)]`, only on fields",
        ));
    }

    let bound: TypeParamBound = parse_quote!(starlark::values::Trace<'v>);
    let mut has_tick_v = false;
    for param in &mut input.generics.params {
        if let GenericParam::Type(type_param) = param {
            if bounds.is_none() {
                type_param.bounds.push(bound.clone());
            }
        }
        if let GenericParam::Lifetime(t) = param {
            if t.lifetime.ident == "v" {
                has_tick_v = true;
            }
        }
    }
    if let Some(bounds) = bounds {
        'outer: for bound in bounds {
            for param in &mut input.generics.params {
                if let GenericParam::Type(type_param) = param {
                    if type_param.ident == bound.ident {
                        type_param.bounds.extend(bound.bounds);
                        continue 'outer;
                    }
                }
            }
            return Err(syn::Error::new_spanned(
                bound,
                "Type parameter not found in the generic parameters",
            ));
        }
    }

    let mut generics2 = input.generics.clone();

    let (_, ty_generics, where_clause) = input.generics.split_for_impl();
    if !has_tick_v {
        generics2.params.insert(0, tick_v);
    }
    let (impl_generics, _, _) = generics2.split_for_impl();

    let name = &input.ident;
    let body = trace_impl(&input, &input.generics)?;
    Ok(syn::parse_quote! {
        unsafe impl #impl_generics starlark::values::Trace<'v> for #name #ty_generics #where_clause {
            #[allow(unused_variables)]
            fn trace(&mut self, tracer: &starlark::values::Tracer<'v>) {
                #body
            }
        }
    })
}

syn::custom_keyword!(unsafe_ignore);
syn::custom_keyword!(bound);

#[derive(Default)]
struct TraceAttrs {
    /// `#[trace(unsafe_ignore)]`
    unsafe_ignore: Option<unsafe_ignore>,
    /// `#[trace(static)]`
    trace_static: Option<syn::Token![static]>,
    /// `#[trace(bound = "A: 'static, B: Trace<'v>")]`
    bounds: Option<Punctuated<syn::TypeParam, syn::Token![,]>>,
}

impl TraceAttrs {
    fn parse(attr: &Attribute) -> syn::Result<TraceAttrs> {
        attr.parse_args_with(|input: ParseStream| {
            let mut trace_attrs = TraceAttrs::default();
            while !input.is_empty() {
                if let Some(unsafe_ignore) = input.parse::<Option<unsafe_ignore>>()? {
                    if trace_attrs.unsafe_ignore.is_some() {
                        return Err(syn::Error::new_spanned(
                            unsafe_ignore,
                            "Duplicate `unsafe_ignore` attribute",
                        ));
                    }
                    trace_attrs.unsafe_ignore = Some(unsafe_ignore);
                } else if let Some(trace_static) = input.parse::<Option<syn::Token![static]>>()? {
                    if trace_attrs.trace_static.is_some() {
                        return Err(syn::Error::new_spanned(
                            trace_static,
                            "Duplicate `static` attribute",
                        ));
                    }
                    trace_attrs.trace_static = Some(trace_static);
                } else if let Some(bound) = input.parse::<Option<bound>>()? {
                    if trace_attrs.bounds.is_some() {
                        return Err(syn::Error::new_spanned(
                            bound,
                            "Duplicate `bound` attribute",
                        ));
                    }
                    input.parse::<syn::Token![=]>()?;
                    let bounds = input.parse::<syn::LitStr>()?;
                    let bounds = bounds
                        .parse_with(|parser: ParseStream| Punctuated::parse_terminated(parser))?;
                    trace_attrs.bounds = Some(bounds);
                } else {
                    return Err(input.error("Unknown attribute"));
                }
                if input.is_empty() {
                    break;
                }
                input.parse::<syn::Token![,]>()?;
            }
            Ok(trace_attrs)
        })
    }
}

/// Parse attribute `#[trace(unsafe_ignore)]`.
///
/// Currently it fails on any attribute argument other than `unsafe_ignore`.
fn parse_attrs(attrs: &[Attribute]) -> syn::Result<TraceAttrs> {
    let mut trace_attrs = None;

    for attr in attrs {
        if attr.path().is_ident("trace") {
            if trace_attrs.is_some() {
                return Err(syn::Error::new_spanned(attr, "Duplicate `trace` attribute"));
            }
            trace_attrs = Some(TraceAttrs::parse(attr)?);
        }
    }

    Ok(trace_attrs.unwrap_or_default())
}

fn trace_impl(derive_input: &DeriveInput, generics: &Generics) -> syn::Result<syn::Expr> {
    let derive_input = DeriveInputUtil::new(derive_input)?;

    let generic_types = generics
        .type_params()
        .map(|x| x.ident.to_string())
        .collect();

    derive_input.for_each_field(|name, field| {
        let TraceAttrs {
            unsafe_ignore,
            trace_static,
            bounds,
        } = parse_attrs(&field.attrs)?;
        if let (Some(unsafe_ignore), Some(_trace_static)) = (unsafe_ignore, trace_static) {
            return Err(syn::Error::new_spanned(
                unsafe_ignore,
                "Cannot have both `unsafe_ignore` and `static` attributes",
            ));
        }
        if let Some(bounds) = bounds {
            return Err(syn::Error::new_spanned(
                bounds,
                "The `bound` attribute can only be used on the `#[derive(Trace)]`",
            ));
        }
        if unsafe_ignore.is_some() {
            Ok(quote! {})
        } else if trace_static.is_some() || is_static(&field.ty, &generic_types) {
            Ok(quote_spanned! {
                field.span()=>
                starlark::values::Tracer::trace_static(tracer, #name);
            })
        } else {
            Ok(quote_spanned! {
                field.span()=>
                starlark::values::Trace::trace(#name, tracer);
            })
        }
    })
}

/// Return `true` if this type definitely satisfies 'static,
/// which means it has no lifetimes that aren't 'static and no generic parameters
/// (as they might have lifetime bounds on them)
fn is_static(ty: &Type, generics: &HashSet<String>) -> bool {
    let f = |x| is_static(x, generics);

    match ty {
        Type::Array(x) => f(&x.elem),
        Type::BareFn(_) => true,
        Type::Never(_) => true,
        Type::Paren(x) => f(&x.elem),
        Type::Path(x) => x.qself.is_none() && is_static_path(&x.path, generics),
        Type::Ptr(_) => true,
        Type::Reference(x) => f(&x.elem) && is_static_opt_lifetime(x.lifetime.as_ref()),
        Type::Slice(x) => f(&x.elem),
        Type::Tuple(x) => x.elems.iter().all(f),
        Type::TraitObject(tr) => {
            let syn::TypeTraitObject {
                dyn_token: _,
                bounds,
            } = tr;
            bounds
                .iter()
                .all(|x| is_static_type_param_bound(x, generics))
        }
        _ => false,
    }
}

fn is_static_path(path: &syn::Path, generics: &HashSet<String>) -> bool {
    path.segments.iter().all(|x| {
        !generics.contains(&x.ident.to_string()) && is_static_path_arguments(&x.arguments, generics)
    })
}

fn is_static_type_param_bound(x: &TypeParamBound, generics: &HashSet<String>) -> bool {
    match x {
        TypeParamBound::Trait(trait_bound) => {
            let TraitBound {
                paren_token: _,
                modifier: _,
                lifetimes,
                path,
            } = trait_bound;
            lifetimes.is_none() && is_static_path(path, generics)
        }
        TypeParamBound::Lifetime(lt) => is_static_lifetime(lt),
        _ => false,
    }
}

fn is_static_path_arguments(x: &PathArguments, generics: &HashSet<String>) -> bool {
    let f = |x| is_static(x, generics);

    match x {
        PathArguments::None => true,
        PathArguments::AngleBracketed(x) => x.args.iter().all(|x| match x {
            GenericArgument::Type(x) => f(x),
            GenericArgument::Lifetime(x) => is_static_opt_lifetime(Some(x)),
            _ => false,
        }),
        PathArguments::Parenthesized(x) => match &x.output {
            ReturnType::Default => false, // An inferred type we can't see
            ReturnType::Type(_, ty) => f(ty) && x.inputs.iter().all(f),
        },
    }
}

fn is_static_lifetime(x: &Lifetime) -> bool {
    x.ident == "static"
}

fn is_static_opt_lifetime(x: Option<&Lifetime>) -> bool {
    match x {
        None => false,
        Some(x) => is_static_lifetime(x),
    }
}
