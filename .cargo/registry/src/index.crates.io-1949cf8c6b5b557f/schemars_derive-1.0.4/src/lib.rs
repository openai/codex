#![forbid(unsafe_code)]
#![deny(unused_imports, clippy::cargo, clippy::pedantic)]
#![allow(
    clippy::result_large_err,
    clippy::wildcard_imports,
    clippy::from_iter_instead_of_collect,
    clippy::too_many_lines
)]

#[macro_use]
extern crate quote;
#[macro_use]
extern crate syn;
extern crate proc_macro;

mod ast;
mod attr;
mod bound;
mod idents;
mod name;
mod schema_exprs;

use ast::Container;
use idents::GENERATOR;
use proc_macro2::TokenStream;
use std::collections::BTreeSet;
use syn::spanned::Spanned;

#[doc = "Derive macro for `JsonSchema` trait."]
#[cfg_attr(not(doctest), doc = include_str!("../deriving.md"), doc = include_str!("../attributes.md"))]
#[proc_macro_derive(JsonSchema, attributes(schemars, serde, validate, garde))]
pub fn derive_json_schema_wrapper(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    derive_json_schema(input, false)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(JsonSchema_repr, attributes(schemars, serde))]
pub fn derive_json_schema_repr_wrapper(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    derive_json_schema(input, true)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn derive_json_schema(mut input: syn::DeriveInput, repr: bool) -> syn::Result<TokenStream> {
    attr::process_serde_attrs(&mut input)?;

    let cont = Container::from_ast(&input)?;

    let crate_alias = cont.attrs.crate_name.as_ref().map(|path| {
        quote_spanned! {path.span()=>
            use #path as schemars;
        }
    });

    let type_name = &cont.ident;

    let (impl_generics, ty_generics, where_clause) = cont.generics.split_for_impl();

    if let Some(ty) = get_transparent_type(&cont) {
        return Ok(quote! {
            const _: () = {
                #crate_alias

                #[automatically_derived]
                impl #impl_generics schemars::JsonSchema for #type_name #ty_generics #where_clause {
                    fn inline_schema() -> bool {
                        <#ty as schemars::JsonSchema>::inline_schema()
                    }

                    fn schema_name() -> schemars::_private::alloc::borrow::Cow<'static, str> {
                        <#ty as schemars::JsonSchema>::schema_name()
                    }

                    fn schema_id() -> schemars::_private::alloc::borrow::Cow<'static, str> {
                        <#ty as schemars::JsonSchema>::schema_id()
                    }

                    fn json_schema(#GENERATOR: &mut schemars::SchemaGenerator) -> schemars::Schema {
                        <#ty as schemars::JsonSchema>::json_schema(#GENERATOR)
                    }

                    fn _schemars_private_non_optional_json_schema(#GENERATOR: &mut schemars::SchemaGenerator) -> schemars::Schema {
                        <#ty as schemars::JsonSchema>::_schemars_private_non_optional_json_schema(#GENERATOR)
                    }

                    fn _schemars_private_is_option() -> bool {
                        <#ty as schemars::JsonSchema>::_schemars_private_is_option()
                    }
                };
            };
        });
    }

    let name = cont.name();
    let const_params = BTreeSet::from_iter(cont.generics.const_params().map(|c| &c.ident));

    // We can't just check if `cont.rename_type_params` is empty, because even if it is, there may
    // be const params in the rename format string
    let schema_name = if cont.attrs.rename_format_string.is_none() || !name.contains('{') {
        quote! {
            schemars::_private::alloc::borrow::Cow::Borrowed(#name)
        }
    } else {
        let type_params = &cont.rename_type_params;

        quote! {
            schemars::_private::alloc::borrow::Cow::Owned(schemars::_private::alloc::format!(
                    #name,
                    #(#type_params=<#type_params as schemars::JsonSchema>::schema_name(),)*
            ))
        }
    };

    let schema_id = if const_params.is_empty() && cont.relevant_type_params.is_empty() {
        quote! {
            schemars::_private::alloc::borrow::Cow::Borrowed(::core::concat!(
                ::core::module_path!(),
                "::",
                #name
            ))
        }
    } else {
        let relevant_type_params = &cont.relevant_type_params;
        let format_string_braces = vec!["{}"; const_params.len() + relevant_type_params.len()];

        quote! {
            schemars::_private::alloc::borrow::Cow::Owned(
                schemars::_private::alloc::format!(
                    ::core::concat!(
                        ::core::module_path!(),
                        "::{}<",
                        #(#format_string_braces,)*
                        ">"
                    ),
                    #name,
                    #(#const_params,)*
                    #(schemars::_schemars_maybe_schema_id!(#relevant_type_params),)*
                )
            )
        }
    };

    let schema_expr = if repr {
        schema_exprs::expr_for_repr(&cont)?
    } else {
        schema_exprs::expr_for_container(&cont)
    };

    let inline = cont.attrs.inline;

    Ok(quote! {
        const _: () = {
            #crate_alias

            #[automatically_derived]
            #[allow(unused_braces)]
            impl #impl_generics schemars::JsonSchema for #type_name #ty_generics #where_clause {
                fn schema_name() -> schemars::_private::alloc::borrow::Cow<'static, str> {
                    #schema_name
                }

                fn schema_id() -> schemars::_private::alloc::borrow::Cow<'static, str> {
                    #schema_id
                }

                fn json_schema(#GENERATOR: &mut schemars::SchemaGenerator) -> schemars::Schema {
                    #schema_expr
                }

                fn inline_schema() -> bool {
                    #inline
                }
            };
        };
    })
}

fn get_transparent_type<'a>(cont: &'a Container) -> Option<&'a syn::Type> {
    // If any schemars attributes for setting metadata (e.g. description) are present, then
    // it's not fully transparent, so use the normal `schema_exprs::expr_for_container`
    // implementation (which always treats the struct as a newtype if it has `transparent`)

    if let Some(attr::WithAttr::Type(ty)) = &cont.attrs.with {
        if cont.attrs.common.is_default() {
            return Some(ty);
        }
    }

    if let Some(transparent_field) = cont.transparent_field() {
        if cont.attrs.common.is_default() && transparent_field.attrs.is_default() {
            return Some(transparent_field.ty);
        }
    }

    None
}
