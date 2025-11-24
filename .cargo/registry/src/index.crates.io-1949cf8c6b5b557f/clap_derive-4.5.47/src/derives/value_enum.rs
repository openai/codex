// Copyright 2018 Guillaume Pinot (@TeXitoi) <texitoi@texitoi.eu>,
// Kevin Knapp (@kbknapp) <kbknapp@gmail.com>, and
// Ana Hobden (@hoverbear) <operator@hoverbear.org>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;
use syn::{spanned::Spanned, Data, DeriveInput, Fields, Ident, Variant};

use crate::item::{Item, Kind, Name};

pub(crate) fn derive_value_enum(input: &DeriveInput) -> Result<TokenStream, syn::Error> {
    let ident = &input.ident;

    match input.data {
        Data::Enum(ref e) => {
            let name = Name::Derived(ident.clone());
            let item = Item::from_value_enum(input, name)?;
            let mut variants = Vec::new();
            for variant in &e.variants {
                let item =
                    Item::from_value_enum_variant(variant, item.casing(), item.env_casing())?;
                variants.push((variant, item));
            }
            gen_for_enum(&item, ident, &variants)
        }
        _ => abort_call_site!("`#[derive(ValueEnum)]` only supports enums"),
    }
}

pub(crate) fn gen_for_enum(
    item: &Item,
    item_name: &Ident,
    variants: &[(&Variant, Item)],
) -> Result<TokenStream, syn::Error> {
    if !matches!(&*item.kind(), Kind::Value) {
        abort! { item.kind().span(),
            "`{}` cannot be used with `value`",
            item.kind().name(),
        }
    }

    let lits = lits(variants)?;
    let value_variants = gen_value_variants(&lits);
    let to_possible_value = gen_to_possible_value(item, &lits);

    Ok(quote! {
        #[allow(
            dead_code,
            unreachable_code,
            unused_variables,
            unused_braces,
            unused_qualifications,
        )]
        #[allow(
            clippy::style,
            clippy::complexity,
            clippy::pedantic,
            clippy::restriction,
            clippy::perf,
            clippy::deprecated,
            clippy::nursery,
            clippy::cargo,
            clippy::suspicious_else_formatting,
            clippy::almost_swapped,
            clippy::redundant_locals,
        )]
        #[automatically_derived]
        impl clap::ValueEnum for #item_name {
            #value_variants
            #to_possible_value
        }
    })
}

fn lits(variants: &[(&Variant, Item)]) -> Result<Vec<(TokenStream, Ident)>, syn::Error> {
    let mut genned = Vec::new();
    for (variant, item) in variants {
        if let Kind::Skip(_, _) = &*item.kind() {
            continue;
        }
        if !matches!(variant.fields, Fields::Unit) {
            abort!(variant.span(), "`#[derive(ValueEnum)]` only supports unit variants. Non-unit variants must be skipped");
        }
        let fields = item.field_methods();
        let deprecations = item.deprecations();
        let name = item.cased_name();
        genned.push((
            quote_spanned! { variant.span()=> {
                #deprecations
                clap::builder::PossibleValue::new(#name)
                #fields
            }},
            variant.ident.clone(),
        ));
    }
    Ok(genned)
}

fn gen_value_variants(lits: &[(TokenStream, Ident)]) -> TokenStream {
    let lit = lits.iter().map(|l| &l.1).collect::<Vec<_>>();

    quote! {
        fn value_variants<'a>() -> &'a [Self]{
            &[#(Self::#lit),*]
        }
    }
}

fn gen_to_possible_value(item: &Item, lits: &[(TokenStream, Ident)]) -> TokenStream {
    let (lit, variant): (Vec<TokenStream>, Vec<Ident>) = lits.iter().cloned().unzip();

    let deprecations = item.deprecations();

    quote! {
        fn to_possible_value<'a>(&self) -> ::std::option::Option<clap::builder::PossibleValue> {
            #deprecations
            match self {
                #(Self::#variant => Some(#lit),)*
                _ => None
            }
        }
    }
}
