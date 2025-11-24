//! Dummy implementations that we emit along with an error.

use proc_macro2::Ident;
use quote::quote;

#[must_use]
pub(crate) fn parser(name: &Ident) -> proc_macro2::TokenStream {
    let into_app = into_app(name);
    quote!(
        #[automatically_derived]
        impl clap::Parser for #name {}
        #into_app
    )
}

#[must_use]
pub(crate) fn into_app(name: &Ident) -> proc_macro2::TokenStream {
    quote! {
        #[automatically_derived]
        impl clap::CommandFactory for #name {
            fn command<'b>() -> clap::Command {
                unimplemented!()
            }
            fn command_for_update<'b>() -> clap::Command {
                unimplemented!()
            }
        }
    }
}

#[must_use]
pub(crate) fn from_arg_matches(name: &Ident) -> proc_macro2::TokenStream {
    quote! {
        #[automatically_derived]
        impl clap::FromArgMatches for #name {
            fn from_arg_matches(_m: &clap::ArgMatches) -> ::std::result::Result<Self, clap::Error> {
                unimplemented!()
            }
            fn update_from_arg_matches(&mut self, matches: &clap::ArgMatches) -> ::std::result::Result<(), clap::Error>{
                unimplemented!()
            }
        }
    }
}

#[must_use]
pub(crate) fn subcommand(name: &Ident) -> proc_macro2::TokenStream {
    let from_arg_matches = from_arg_matches(name);
    quote! {
        #[automatically_derived]
        impl clap::Subcommand for #name {
            fn augment_subcommands(_cmd: clap::Command) -> clap::Command {
                unimplemented!()
            }
            fn augment_subcommands_for_update(_cmd: clap::Command) -> clap::Command {
                unimplemented!()
            }
            fn has_subcommand(name: &str) -> bool {
                unimplemented!()
            }
        }
        #from_arg_matches
    }
}

#[must_use]
pub(crate) fn args(name: &Ident) -> proc_macro2::TokenStream {
    let from_arg_matches = from_arg_matches(name);
    quote! {
        #[automatically_derived]
        impl clap::Args for #name {
            fn augment_args(_cmd: clap::Command) -> clap::Command {
                unimplemented!()
            }
            fn augment_args_for_update(_cmd: clap::Command) -> clap::Command {
                unimplemented!()
            }
        }
        #from_arg_matches
    }
}

#[must_use]
pub(crate) fn value_enum(name: &Ident) -> proc_macro2::TokenStream {
    quote! {
        #[automatically_derived]
        impl clap::ValueEnum for #name {
            fn value_variants<'a>() -> &'a [Self]{
                unimplemented!()
            }
            fn from_str(_input: &str, _ignore_case: bool) -> ::std::result::Result<Self, String> {
                unimplemented!()
            }
            fn to_possible_value<'a>(&self) -> ::std::option::Option<clap::builder::PossibleValue>{
                unimplemented!()
            }
        }
    }
}
