/*
 * Copyright 2018 The Starlark in Rust Authors.
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
use proc_macro2::TokenTree;
use quote::format_ident;
use quote::quote;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::token;
use syn::Data;
use syn::DeriveInput;
use syn::Expr;
use syn::Field;
use syn::GenericArgument;
use syn::Ident;
use syn::Path;
use syn::PathArguments;
use syn::ReturnType;
use syn::Token;
use syn::Type;

// This macro does two related derivations depending on whether there are any generic parameters.
//
// struct A<T>(...) => coerce A<T1> to A<T2> if coerce T1 to T2 and all the fields support it
pub fn derive_coerce(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match derive_coerce_impl(input) {
        Ok(x) => x.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn derive_coerce_impl(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    check_repr(&input)?;

    if input.generics.type_params().count() == 0 {
        Err(syn::Error::new_spanned(
            &input,
            "`Coerce` can only be derived for types with type parameters",
        ))
    } else {
        derive_coerce_params(input)
    }
}

#[derive(Copy, Clone)]
enum ParamNameMapping {
    From,
    To,
}

impl ParamNameMapping {
    fn format_ident(self, ident: &Ident) -> Ident {
        match self {
            ParamNameMapping::From => format_ident!("From{}", ident),
            ParamNameMapping::To => format_ident!("To{}", ident),
        }
    }
}

fn derive_coerce_params(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let lifetimes = input.generics.lifetimes().collect::<Vec<_>>();

    // unsafe impl <To__T, From__T> Coerce<X<To__T>> X<From__T> where ...
    let name = &input.ident;
    let mut ty_args = HashSet::new();
    let mut ty_args_to = Vec::new();
    let mut ty_args_from = Vec::new();
    let mut ty_args_to1 = Vec::new();
    let mut ty_args_from1 = Vec::new();
    for t in input.generics.type_params() {
        ty_args.insert(t.ident.clone());
        let to_ident = ParamNameMapping::To.format_ident(&t.ident);
        let from_ident = ParamNameMapping::From.format_ident(&t.ident);
        ty_args_to.push(to_ident.clone());
        ty_args_from.push(from_ident.clone());
        if t.bounds.is_empty() {
            ty_args_to1.push(quote! { #to_ident });
            ty_args_from1.push(quote! { #from_ident });
        } else {
            let bounds = &t.bounds;
            ty_args_to1.push(quote! { #to_ident: #bounds });
            ty_args_from1.push(quote! { #from_ident: #bounds });
        }
    }

    let mut constraints = Vec::new();
    let fields = collect_fields(&input)?;
    for x in fields {
        let mut to_ty = x.ty.clone();
        let mut from_ty = x.ty.clone();
        replace_type(&mut to_ty, &ty_args, ParamNameMapping::To)?;
        replace_type(&mut from_ty, &ty_args, ParamNameMapping::From)?;
        if to_ty != from_ty {
            constraints.push(quote! { #from_ty : starlark::coerce::Coerce< #to_ty >});
        }
    }

    Ok(quote! {
        unsafe impl < #(#lifetimes,)* #(#ty_args_from1,)* #(#ty_args_to1,)* >
            starlark::coerce::Coerce<#name < #(#lifetimes,)* #(#ty_args_to,)* >>
            for #name < #(#lifetimes,)* #(#ty_args_from,)* >
            where #(#constraints,)* {}
    })
}

fn collect_fields(input: &DeriveInput) -> syn::Result<Vec<&Field>> {
    match &input.data {
        Data::Struct(x) => Ok(x.fields.iter().collect()),
        Data::Enum(x) => Ok(x.variants.iter().flat_map(|x| &x.fields).collect()),
        Data::Union(..) => Err(syn::Error::new_spanned(
            input,
            "Type-parameter cannot be a union",
        )),
    }
}

fn replace_type(
    ty: &mut Type,
    idents: &HashSet<Ident>,
    mapping: ParamNameMapping,
) -> syn::Result<()> {
    match ty {
        Type::Path(x)
            if x.qself.is_none()
                && x.path.segments.len() == 1
                && x.path.segments[0].arguments.is_empty() =>
        {
            let i = &mut x.path.segments[0].ident;
            if idents.contains(i) {
                *i = mapping.format_ident(i);
            }
            Ok(())
        }
        _ => descend_type(ty, |ty| replace_type(ty, idents, mapping)),
    }
}

/// Descend into all the nested type values within a type,
/// or return an error if you don't know how.
fn descend_type(ty: &mut Type, op: impl Fn(&mut Type) -> syn::Result<()>) -> syn::Result<()> {
    match ty {
        Type::Array(x) => op(&mut x.elem),
        Type::Group(x) => op(&mut x.elem),
        Type::Never(_) => Ok(()),
        Type::Paren(x) => op(&mut x.elem),
        Type::Path(x) => {
            if let Some(qself) = &mut x.qself {
                op(&mut qself.ty)?;
            }
            for p in x.path.segments.iter_mut() {
                match &mut p.arguments {
                    PathArguments::None => {}
                    PathArguments::AngleBracketed(x) => {
                        x.args.iter_mut().try_for_each(|x| match x {
                            GenericArgument::Type(x) => op(x),
                            _ => Ok(()),
                        })?
                    }
                    PathArguments::Parenthesized(x) => {
                        x.inputs.iter_mut().try_for_each(&op)?;
                        if let ReturnType::Type(_, ty) = &mut x.output {
                            op(ty)?;
                        }
                    }
                }
            }
            Ok(())
        }
        Type::Ptr(x) => op(&mut x.elem),
        Type::Reference(x) => op(&mut x.elem),
        Type::Slice(x) => op(&mut x.elem),
        Type::Tuple(xs) => xs.elems.iter_mut().try_for_each(op),
        _ => Err(syn::Error::new_spanned(ty, "Unsupported type")),
    }
}

// Taken from ref_cast: https://github.com/dtolnay/ref-cast/blob/eadd839cc0db116e4fc9bfee5dd29dde82089eed/derive/src/lib.rs#L91
fn check_repr(input: &DeriveInput) -> syn::Result<()> {
    let mut has_repr = false;
    let mut errors = None;
    let mut push_error = |error| match &mut errors {
        Some(errors) => syn::Error::combine(errors, error),
        None => errors = Some(error),
    };

    for attr in &input.attrs {
        if attr.path().is_ident("repr") {
            if let Err(error) = attr.parse_args_with(|input: ParseStream| {
                while !input.is_empty() {
                    let path = input.call(Path::parse_mod_style)?;
                    if path.is_ident("C") || path.is_ident("transparent") {
                        has_repr = true;
                    } else if path.is_ident("packed") {
                        // ignore
                    } else {
                        let meta_item_span = if input.peek(token::Paren) {
                            let group: TokenTree = input.parse()?;
                            quote!(#path #group)
                        } else if input.peek(Token![=]) {
                            let eq_token: Token![=] = input.parse()?;
                            let value: Expr = input.parse()?;
                            quote!(#path #eq_token #value)
                        } else {
                            quote!(#path)
                        };
                        let msg = if path.is_ident("align") {
                            "aligned repr on struct that implements Coerce is not supported"
                        } else {
                            "unrecognized repr on struct that implements Coerce"
                        };
                        push_error(syn::Error::new_spanned(meta_item_span, msg));
                    }
                    if !input.is_empty() {
                        input.parse::<Token![,]>()?;
                    }
                }
                Ok(())
            }) {
                push_error(error);
            }
        }
    }

    if !has_repr {
        let mut requires_repr = syn::Error::new(
            Span::call_site(),
            "Coerce trait requires #[repr(C)] or #[repr(transparent)]",
        );
        if let Some(errors) = errors {
            requires_repr.combine(errors);
        }
        errors = Some(requires_repr);
    }

    match errors {
        None => Ok(()),
        Some(errors) => Err(errors),
    }
}
