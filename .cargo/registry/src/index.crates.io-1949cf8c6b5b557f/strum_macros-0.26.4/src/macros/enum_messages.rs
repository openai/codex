use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, LitStr};

use crate::helpers::{non_enum_error, HasStrumVariantProperties, HasTypeProperties};

pub fn enum_message_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => return Err(non_enum_error()),
    };

    let type_properties = ast.get_type_properties()?;
    let strum_module_path = type_properties.crate_module_path();

    let mut arms = Vec::new();
    let mut detailed_arms = Vec::new();
    let mut documentation_arms = Vec::new();
    let mut serializations = Vec::new();

    for variant in variants {
        let variant_properties = variant.get_variant_properties()?;
        let messages = variant_properties.message.as_ref();
        let detailed_messages = variant_properties.detailed_message.as_ref();
        let documentation = &variant_properties.documentation;
        let ident = &variant.ident;

        let params = match variant.fields {
            Fields::Unit => quote! {},
            Fields::Unnamed(..) => quote! { (..) },
            Fields::Named(..) => quote! { {..} },
        };

        // You can't disable getting the serializations.
        {
            let serialization_variants =
                variant_properties.get_serializations(type_properties.case_style);

            let count = serialization_variants.len();
            serializations.push(quote! {
                &#name::#ident #params => {
                    static ARR: [&'static str; #count] = [#(#serialization_variants),*];
                    &ARR
                }
            });
        }

        // But you can disable the messages.
        if variant_properties.disabled.is_some() {
            continue;
        }

        if let Some(msg) = messages {
            let params = params.clone();

            // Push the simple message.
            let tokens = quote! { &#name::#ident #params => ::core::option::Option::Some(#msg) };
            arms.push(tokens.clone());

            if detailed_messages.is_none() {
                detailed_arms.push(tokens);
            }
        }

        if let Some(msg) = detailed_messages {
            let params = params.clone();
            // Push the detailed message.
            detailed_arms
                .push(quote! { &#name::#ident #params => ::core::option::Option::Some(#msg) });
        }

        if !documentation.is_empty() {
            let params = params.clone();
            // Strip a single leading space from each documentation line.
            let documentation: Vec<LitStr> = documentation
                .iter()
                .map(|lit_str| {
                    let line = lit_str.value();
                    if line.starts_with(' ') {
                        LitStr::new(&line.as_str()[1..], lit_str.span())
                    } else {
                        lit_str.clone()
                    }
                })
                .collect();
            if documentation.len() == 1 {
                let text = &documentation[0];
                documentation_arms
                    .push(quote! { &#name::#ident #params => ::core::option::Option::Some(#text) });
            } else {
                // Push the documentation.
                documentation_arms
                    .push(quote! {
                        &#name::#ident #params => ::core::option::Option::Some(concat!(#(concat!(#documentation, "\n")),*))
                    });
            }
        }
    }

    if arms.len() < variants.len() {
        arms.push(quote! { _ => ::core::option::Option::None });
    }

    if detailed_arms.len() < variants.len() {
        detailed_arms.push(quote! { _ => ::core::option::Option::None });
    }

    if documentation_arms.len() < variants.len() {
        documentation_arms.push(quote! { _ => ::core::option::Option::None });
    }

    Ok(quote! {
        impl #impl_generics #strum_module_path::EnumMessage for #name #ty_generics #where_clause {
            fn get_message(&self) -> ::core::option::Option<&'static str> {
                match self {
                    #(#arms),*
                }
            }

            fn get_detailed_message(&self) -> ::core::option::Option<&'static str> {
                match self {
                    #(#detailed_arms),*
                }
            }

            fn get_documentation(&self) -> ::core::option::Option<&'static str> {
                match self {
                    #(#documentation_arms),*
                }
            }

            fn get_serializations(&self) -> &'static [&'static str] {
                match self {
                    #(#serializations),*
                }
            }
        }
    })
}
