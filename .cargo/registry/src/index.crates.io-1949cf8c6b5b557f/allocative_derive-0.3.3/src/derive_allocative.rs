/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use proc_macro2::Ident;
use proc_macro2::Span;
use quote::quote_spanned;
use quote::ToTokens;
use syn::parse::ParseStream;
use syn::spanned::Spanned;
use syn::Attribute;
use syn::Data;
use syn::DataEnum;
use syn::DataStruct;
use syn::DeriveInput;
use syn::Field;
use syn::Fields;
use syn::FieldsNamed;
use syn::FieldsUnnamed;
use syn::GenericParam;
use syn::Generics;
use syn::LitStr;
use syn::Path;
use syn::Token;
use syn::Variant;

const fn hash(s: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325;
    let mut i = 0;
    while i < s.as_bytes().len() {
        let b = s.as_bytes()[i];
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        i += 1;
    }
    hash
}

pub(crate) fn derive_allocative(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match derive_allocative_impl(input.into()) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn impl_generics(
    generics: &Generics,
    attrs: &AllocativeAttrs,
) -> syn::Result<proc_macro2::TokenStream> {
    if let Some(bound) = &attrs.bound {
        if !bound.is_empty() {
            let span = attrs.span.unwrap_or_else(Span::call_site);
            let bound = bound.parse::<proc_macro2::TokenStream>()?;
            return Ok(quote_spanned! { span => < #bound > });
        }
    }

    let mut impl_generics = Vec::new();
    for p in &generics.params {
        impl_generics.push(match p {
            GenericParam::Type(tp) => {
                let mut tp = tp.clone();
                if attrs.bound.is_none() && !attrs.skip {
                    tp.bounds.push(syn::parse2(quote_spanned! { tp.span() =>
                        allocative::Allocative
                    })?);
                }
                tp.default = None;
                tp.to_token_stream()
            }
            GenericParam::Lifetime(l) => l.to_token_stream(),
            GenericParam::Const(c) => c.to_token_stream(),
        });
    }
    if impl_generics.is_empty() {
        Ok(quote_spanned! { generics.span() => })
    } else {
        Ok(quote_spanned! { generics.span() => <#(#impl_generics),*> })
    }
}

fn derive_allocative_impl(
    input: proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let input: DeriveInput = syn::parse2(input)?;
    let name = &input.ident;
    let (_impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    let attrs = extract_attrs(&input.attrs)?;
    let impl_generics = impl_generics(&input.generics, &attrs)?;

    let body = if attrs.skip {
        quote_spanned! { input.span() =>
        }
    } else {
        gen_visit_body(&input)?
    };

    Ok(quote_spanned! {input.span()=>
        impl #impl_generics allocative::Allocative for #name #type_generics #where_clause {
            #[allow(unused, warnings)]
            fn visit<'allocative_a, 'allocative_b: 'allocative_a>(
                &self,
                visitor: &'allocative_a mut allocative::Visitor<'allocative_b>,
            ) {
                let mut visitor = visitor.enter_self::<Self>(self);
                #body
                visitor.exit();
            }
        }
    })
}

fn gen_visit_body(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    match &input.data {
        Data::Struct(s) => gen_visit_struct(s),
        Data::Union(_) => Err(syn::Error::new_spanned(input, "unions are not supported")),
        Data::Enum(e) => gen_visit_enum(e),
    }
}

fn gen_visit_enum(input: &DataEnum) -> syn::Result<proc_macro2::TokenStream> {
    let cases = input
        .variants
        .iter()
        .map(gen_visit_enum_variant)
        .collect::<syn::Result<proc_macro2::TokenStream>>()?;
    if cases.is_empty() {
        // Rust does not understand
        // ```
        // match self {
        // }
        // ```
        // for enums with no variants where `self` is `&Self`.
        Ok(quote_spanned! {input.variants.span()=>
        })
    } else {
        Ok(quote_spanned! {input.variants.span()=>
        match self {
                #cases
            }
        })
    }
}

fn allocative_key(s: &str) -> proc_macro2::TokenStream {
    // Compile hash at proc macro time, otherwise it will have to be computed by MIRI.
    let hash = hash(s);
    quote_spanned! {proc_macro2::Span::call_site()=>
        allocative::Key::new_unchecked(#hash, #s)
    }
}

fn gen_visit_enum_variant(input: &Variant) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let name_str = name.to_string();

    let variant_attrs = extract_attrs(&input.attrs)?;

    let variant_key = allocative_key(&name_str);

    // TODO: enter variant.
    match &input.fields {
        Fields::Unit => Ok(quote_spanned! {input.span()=>
            Self::#name => {},
        }),
        Fields::Unnamed(unnamed) => {
            let field_names = fields_unnamed_names(unnamed)?;
            let visit_fields = unnamed
                .unnamed
                .iter()
                .enumerate()
                .zip(field_names.iter())
                .map(|((i, f), n)| {
                    if variant_attrs.skip {
                        Ok(quote_spanned! {f.span()=>
                        })
                    } else {
                        gen_visit_field(&i.to_string(), n, f)
                    }
                })
                .collect::<syn::Result<proc_macro2::TokenStream>>()?;
            Ok(quote_spanned! {input.span()=>
                Self::#name(#(#field_names),*) => {
                    let mut visitor = visitor.enter(#variant_key, std::mem::size_of::<Self>());
                    #visit_fields
                    visitor.exit();
                },
            })
        }
        Fields::Named(named) => {
            let field_names = fields_named_names(named)?;
            let visit_fields = field_names
                .iter()
                .zip(named.named.iter())
                .map(|(ident, f)| {
                    if variant_attrs.skip {
                        Ok(quote_spanned! {f.span()=>
                        })
                    } else {
                        gen_visit_field(&ident.to_string(), ident, f)
                    }
                })
                .collect::<syn::Result<proc_macro2::TokenStream>>()?;
            Ok(quote_spanned! {input.span()=>
                Self::#name { #(#field_names),* } => {
                    let mut visitor = visitor.enter(#variant_key, std::mem::size_of::<Self>());
                    #visit_fields
                    visitor.exit();
                },
            })
        }
    }
}

fn fields_named_names(fields: &FieldsNamed) -> syn::Result<Vec<&Ident>> {
    fields
        .named
        .iter()
        .map(|f| {
            f.ident
                .as_ref()
                .ok_or_else(|| syn::Error::new_spanned(f, "unnamed field in named"))
        })
        .collect()
}

fn fields_unnamed_names(fields: &FieldsUnnamed) -> syn::Result<Vec<Ident>> {
    fields
        .unnamed
        .iter()
        .enumerate()
        .map(|(i, f)| Ok(Ident::new(&format!("f{}", i), f.span())))
        .collect()
}

fn gen_visit_struct(input: &DataStruct) -> syn::Result<proc_macro2::TokenStream> {
    match &input.fields {
        Fields::Named(named) => {
            let names = fields_named_names(named)?;
            let visit_fields = names
                .iter()
                .zip(named.named.iter())
                .map(|(ident, f)| gen_visit_field(&ident.to_string(), ident, f))
                .collect::<syn::Result<proc_macro2::TokenStream>>()?;
            Ok(quote_spanned! {input.fields.span()=>
                let Self { #(#names),* } = self;
                #visit_fields
            })
        }
        Fields::Unnamed(unnamed) => {
            let names = fields_unnamed_names(unnamed)?;
            let visit_fields = names
                .iter()
                .enumerate()
                .zip(unnamed.unnamed.iter())
                .map(|((i, ident), field)| gen_visit_field(&i.to_string(), ident, field))
                .collect::<syn::Result<proc_macro2::TokenStream>>()?;
            Ok(quote_spanned! {input.fields.span()=>
                let Self(#(#names),*) = self;
                #visit_fields
            })
        }
        Fields::Unit => Ok(quote_spanned! {input.fields.span()=>}),
    }
}

fn gen_visit_field(
    label: &str,
    ident: &Ident,
    field: &Field,
) -> syn::Result<proc_macro2::TokenStream> {
    let attrs = extract_attrs(&field.attrs)?;
    let field_key = allocative_key(label);
    if attrs.skip {
        Ok(quote_spanned! {field.span()=>})
    } else if let Some(visit) = attrs.visit {
        let ty = &field.ty;
        Ok(quote_spanned! {field.span()=>
            // TODO(nga): figure out how to put this snippet in a member function of the visitor.
            {
                let mut visitor = visitor.enter(#field_key, std::mem::size_of::<#ty>());
                #visit(#ident, &mut visitor);
                visitor.exit();
            }
        })
    } else {
        // Specify type parameter explicitly to prevent implicit conversion.
        let ty = &field.ty;
        Ok(quote_spanned! {ident.span()=>
            visitor.visit_field::<#ty>(#field_key, #ident);
        })
    }
}

#[derive(Default)]
struct AllocativeAttrs {
    span: Option<Span>,
    skip: bool,
    bound: Option<String>,
    visit: Option<Path>,
}

/// Parse an `#[allocative(...)]` annotation.
fn extract_attrs(attrs: &[Attribute]) -> syn::Result<AllocativeAttrs> {
    syn::custom_keyword!(skip);
    syn::custom_keyword!(bound);
    syn::custom_keyword!(visit);

    let mut opts = AllocativeAttrs::default();

    for attr in attrs.iter() {
        if !attr.path().is_ident("allocative") {
            continue;
        }

        opts.span = Some(attr.span());

        attr.parse_args_with(|input: ParseStream| {
            loop {
                if input.parse::<skip>().is_ok() {
                    if opts.skip {
                        return Err(input.error("`skip` was set twice"));
                    }
                    opts.skip = true;
                } else if input.parse::<bound>().is_ok() {
                    input.parse::<Token![=]>()?;
                    let bound = input.parse::<LitStr>()?;
                    if opts.bound.is_some() {
                        return Err(input.error("`bound` was set twice"));
                    }
                    opts.bound = Some(bound.value());
                } else if input.parse::<visit>().is_ok() {
                    input.parse::<Token![=]>()?;
                    let visit = input.parse::<Path>()?;
                    if opts.visit.is_some() {
                        return Err(input.error("`visit` was set twice"));
                    }
                    opts.visit = Some(visit);
                }

                if input.is_empty() {
                    return Ok(());
                }

                input.parse::<Token![,]>()?;
            }
        })?;
    }

    Ok(opts)
}
