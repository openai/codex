use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Lit};

use crate::helpers::{non_enum_error, HasStrumVariantProperties, HasTypeProperties};

#[derive(Hash, PartialEq, Eq)]
enum PropertyType {
    String,
    Integer,
    Bool,
}

const PROPERTY_TYPES: [PropertyType; 3] = [
    PropertyType::String,
    PropertyType::Integer,
    PropertyType::Bool,
];

pub fn enum_properties_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => return Err(non_enum_error()),
    };
    let type_properties = ast.get_type_properties()?;
    let strum_module_path = type_properties.crate_module_path();

    let mut built_arms: HashMap<_, _> = PROPERTY_TYPES.iter().map(|p| (p, Vec::new())).collect();

    for variant in variants {
        let ident = &variant.ident;
        let variant_properties = variant.get_variant_properties()?;
        let mut arms: HashMap<_, _> = PROPERTY_TYPES.iter().map(|p| (p, Vec::new())).collect();
        // But you can disable the messages.
        if variant_properties.disabled.is_some() {
            continue;
        }

        let params = match variant.fields {
            Fields::Unit => quote! {},
            Fields::Unnamed(..) => quote! { (..) },
            Fields::Named(..) => quote! { {..} },
        };

        for (key, value) in variant_properties.props {
            let property_type = match value {
                Lit::Str(..) => PropertyType::String,
                Lit::Bool(..) => PropertyType::Bool,
                Lit::Int(..) => PropertyType::Integer,
                _ => todo!("TODO"),
            };

            arms.get_mut(&property_type)
                .unwrap()
                .push(quote! { #key => ::core::option::Option::Some( #value )});
        }

        for property in &PROPERTY_TYPES {
            arms.get_mut(&property)
                .unwrap()
                .push(quote! { _ => ::core::option::Option::None });
            let arms_as_string = &arms[property];
            built_arms.get_mut(&property).unwrap().push(quote! {
                &#name::#ident #params => {
                    match prop {
                        #(#arms_as_string),*
                    }
                }
            });
        }
    }

    for (_, arms) in built_arms.iter_mut() {
        if arms.len() < variants.len() {
            arms.push(quote! { _ => ::core::option::Option::None });
        }
    }

    let (built_string_arms, built_int_arms, built_bool_arms) = (
        &built_arms[&PropertyType::String],
        &built_arms[&PropertyType::Integer],
        &built_arms[&PropertyType::Bool],
    );

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #strum_module_path::EnumProperty for #name #ty_generics #where_clause {
            #[inline]
            fn get_str(&self, prop: &str) -> ::core::option::Option<&'static str> {
                match self {
                    #(#built_string_arms),*
                }
            }

            #[inline]
            fn get_int(&self, prop: &str) -> ::core::option::Option<i64> {
                match self {
                    #(#built_int_arms),*
                }
            }

            #[inline]
            fn get_bool(&self, prop: &str) -> ::core::option::Option<bool> {
                match self {
                    #(#built_bool_arms),*
                }
            }

        }
    })
}
