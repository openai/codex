use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{punctuated::Punctuated, Data, DeriveInput, Fields, LitStr, Token};

use crate::helpers::{
    non_enum_error, non_single_field_variant_error, HasStrumVariantProperties, HasTypeProperties,
};

pub fn display_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => return Err(non_enum_error()),
    };

    let type_properties = ast.get_type_properties()?;

    let mut arms = Vec::new();
    for variant in variants {
        let ident = &variant.ident;
        let variant_properties = variant.get_variant_properties()?;

        if variant_properties.disabled.is_some() {
            continue;
        }

        if let Some(..) = variant_properties.transparent {
            let arm = super::extract_single_field_variant_and_then(name, variant, |tok| {
                quote! { ::core::fmt::Display::fmt(#tok, f) }
            })
            .map_err(|_| non_single_field_variant_error("transparent"))?;

            arms.push(arm);
            continue;
        }

        // Look at all the serialize attributes.
        let output = variant_properties.get_preferred_name(
            type_properties.case_style,
            type_properties.prefix.as_ref(),
            type_properties.suffix.as_ref(),
        );

        let params = match variant.fields {
            Fields::Unit => quote! {},
            Fields::Unnamed(ref unnamed_fields) => {
                // Transform unnamed params '(String, u8)' to '(ref field0, ref field1)'
                let names: Punctuated<_, Token!(,)> = unnamed_fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        assert!(field.ident.is_none());
                        let ident =
                            syn::parse_str::<Ident>(format!("field{}", index).as_str()).unwrap();
                        quote! { ref #ident }
                    })
                    .collect();
                quote! { (#names) }
            }
            Fields::Named(ref field_names) => {
                // Transform named params '{ name: String, age: u8 }' to '{ ref name, ref age }'
                let names: Punctuated<TokenStream, Token!(,)> = field_names
                    .named
                    .iter()
                    .map(|field| {
                        let ident = field.ident.as_ref().unwrap();
                        quote! { ref #ident }
                    })
                    .collect();

                quote! { {#names} }
            }
        };

        if variant_properties.to_string.is_none() && variant_properties.default.is_some() {
            let arm = super::extract_single_field_variant_and_then(name, variant, |tok| {
                quote! { ::core::fmt::Display::fmt(#tok, f)}
            })
            .map_err(|_| {
                syn::Error::new_spanned(
                    variant,
                    "Default only works on newtype structs with a single String field",
                )
            })?;

            arms.push(arm);
            continue;
        }

        let arm = match variant.fields {
            Fields::Named(ref field_names) => {
                let used_vars = capture_format_string_idents(&output)?;
                if used_vars.is_empty() {
                    quote! { #name::#ident #params => ::core::fmt::Display::fmt(#output, f) }
                } else {
                    // Create args like 'name = name, age = age' for format macro
                    let args: Punctuated<_, Token!(,)> = field_names
                        .named
                        .iter()
                        .filter_map(|field| {
                            let ident = field.ident.as_ref().unwrap();
                            // Only contain variables that are used in format string
                            if !used_vars.contains(ident) {
                                None
                            } else {
                                Some(quote! { #ident = #ident })
                            }
                        })
                        .collect();

                    quote! {
                        #[allow(unused_variables)]
                        #name::#ident #params => ::core::fmt::Display::fmt(&format_args!(#output, #args), f)
                    }
                }
            }
            Fields::Unnamed(ref unnamed_fields) => {
                let used_vars = capture_format_strings(&output)?;
                if used_vars.iter().any(String::is_empty) {
                    return Err(syn::Error::new_spanned(
                        &output,
                        "Empty {} is not allowed; Use manual numbering ({0})",
                    ));
                }
                if used_vars.is_empty() {
                    quote! { #name::#ident #params => ::core::fmt::Display::fmt(#output, f) }
                } else {
                    let args: Punctuated<_, Token!(,)> = unnamed_fields
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(index, field)| {
                            assert!(field.ident.is_none());
                            syn::parse_str::<Ident>(format!("field{}", index).as_str()).unwrap()
                        })
                        .collect();
                    quote! {
                        #[allow(unused_variables)]
                        #name::#ident #params => ::core::fmt::Display::fmt(&format!(#output, #args), f)
                    }
                }
            }
            Fields::Unit => {
                let used_vars = capture_format_strings(&output)?;
                if !used_vars.is_empty() {
                    return Err(syn::Error::new_spanned(
                        &output,
                        "Unit variants do not support interpolation",
                    ));
                }

                quote! { #name::#ident #params => ::core::fmt::Display::fmt(#output, f) }
            }
        };

        arms.push(arm);
    }

    if arms.len() < variants.len() {
        arms.push(quote! { _ => panic!("fmt() called on disabled variant.") });
    }

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::core::fmt::Display for #name #ty_generics #where_clause {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::result::Result<(), ::core::fmt::Error> {
                match *self {
                    #(#arms),*
                }
            }
        }
    })
}

fn capture_format_string_idents(string_literal: &LitStr) -> syn::Result<Vec<Ident>> {
    capture_format_strings(string_literal)?
        .into_iter()
        .map(|ident| {
            syn::parse_str::<Ident>(ident.as_str()).map_err(|_| {
                syn::Error::new_spanned(
                    string_literal,
                    "Invalid identifier inside format string bracket",
                )
            })
        })
        .collect()
}

fn capture_format_strings(string_literal: &LitStr) -> syn::Result<Vec<String>> {
    // Remove escaped brackets
    let format_str = string_literal.value().replace("{{", "").replace("}}", "");

    let mut new_var_start_index: Option<usize> = None;
    let mut var_used = Vec::new();

    for (i, chr) in format_str.bytes().enumerate() {
        if chr == b'{' {
            if new_var_start_index.is_some() {
                return Err(syn::Error::new_spanned(
                    string_literal,
                    "Bracket opened without closing previous bracket",
                ));
            }
            new_var_start_index = Some(i);
            continue;
        }

        if chr == b'}' {
            let start_index = new_var_start_index.take().ok_or(syn::Error::new_spanned(
                string_literal,
                "Bracket closed without previous opened bracket",
            ))?;

            let inside_brackets = &format_str[start_index + 1..i];
            let ident_str = inside_brackets.split(":").next().unwrap().trim_end();
            var_used.push(ident_str.to_owned());
        }
    }

    Ok(var_used)
}
