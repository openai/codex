// Copyright 2018 Guillaume Pinot (@TeXitoi) <texitoi@texitoi.eu>,
// Kevin Knapp (@kbknapp) <kbknapp@gmail.com>, and
// Ana Hobden (@hoverbear) <operator@hoverbear.org>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// This work was derived from Structopt (https://github.com/TeXitoi/structopt)
// commit#ea76fa1b1b273e65e3b0b1046643715b49bec51f which is licensed under the
// MIT/Apache 2.0 license.

#![doc = include_str!("../README.md")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/clap-rs/clap/master/assets/clap.png")]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};
use syn::{Data, DataStruct, Fields};

#[macro_use]
mod macros;

mod attr;
mod derives;
mod dummies;
mod item;
mod utils;

/// Generates the `ValueEnum` impl.
#[proc_macro_derive(ValueEnum, attributes(clap, value))]
pub fn value_enum(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);
    derives::derive_value_enum(&input)
        .unwrap_or_else(|err| {
            let dummy = dummies::value_enum(&input.ident);
            to_compile_error(err, dummy)
        })
        .into()
}

/// Generates the `Parser` implementation.
///
/// This is far less verbose than defining the `clap::Command` struct manually,
/// receiving an instance of `clap::ArgMatches` from conducting parsing, and then
/// implementing a conversion code to instantiate an instance of the user
/// context struct.
#[proc_macro_derive(Parser, attributes(clap, structopt, command, arg, group))]
pub fn parser(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);
    derives::derive_parser(&input)
        .unwrap_or_else(|err| {
            let specific_dummy = match input.data {
                Data::Struct(DataStruct {
                    fields: Fields::Named(ref _fields),
                    ..
                }) => Some(dummies::args(&input.ident)),
                Data::Struct(DataStruct {
                    fields: Fields::Unit,
                    ..
                }) => Some(dummies::args(&input.ident)),
                Data::Enum(_) => Some(dummies::subcommand(&input.ident)),
                _ => None,
            };
            let dummy = specific_dummy
                .map(|specific_dummy| {
                    let parser_dummy = dummies::parser(&input.ident);
                    quote::quote! {
                        #parser_dummy
                        #specific_dummy
                    }
                })
                .unwrap_or_else(|| quote::quote!());
            to_compile_error(err, dummy)
        })
        .into()
}

/// Generates the `Subcommand` impl.
#[proc_macro_derive(Subcommand, attributes(clap, command, arg, group))]
pub fn subcommand(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);
    derives::derive_subcommand(&input)
        .unwrap_or_else(|err| {
            let dummy = dummies::subcommand(&input.ident);
            to_compile_error(err, dummy)
        })
        .into()
}

/// Generates the `Args` impl.
#[proc_macro_derive(Args, attributes(clap, command, arg, group))]
pub fn args(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);
    derives::derive_args(&input)
        .unwrap_or_else(|err| {
            let dummy = dummies::args(&input.ident);
            to_compile_error(err, dummy)
        })
        .into()
}

fn to_compile_error(
    error: syn::Error,
    dummy: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let compile_errors = error.to_compile_error();
    quote::quote!(
        #dummy
        #compile_errors
    )
}
