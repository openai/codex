use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, Data, DeriveInput, Fields};

use crate::helpers::{
    non_enum_error, non_single_field_variant_error, HasStrumVariantProperties, HasTypeProperties,
};

fn get_arms<F>(ast: &DeriveInput, transparent_fn: F) -> syn::Result<Vec<TokenStream>>
where
    F: Fn(&TokenStream) -> TokenStream,
{
    let name = &ast.ident;
    let mut arms = Vec::new();
    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => return Err(non_enum_error()),
    };

    let type_properties = ast.get_type_properties()?;

    for variant in variants {
        let ident = &variant.ident;
        let variant_properties = variant.get_variant_properties()?;

        if variant_properties.disabled.is_some() {
            continue;
        }

        if let Some(..) = variant_properties.transparent {
            let arm = super::extract_single_field_variant_and_then(name, variant, |tok| {
                transparent_fn(tok)
            })
            .map_err(|_| non_single_field_variant_error("transparent"))?;

            arms.push(arm);
            continue;
        }

        // Look at all the serialize attributes.
        // Use `to_string` attribute (not `as_ref_str` or something) to keep things consistent
        // (i.e. always `enum.as_ref().to_string() == enum.to_string()`).
        let output = variant_properties.get_preferred_name(
            type_properties.case_style,
            type_properties.prefix.as_ref(),
            type_properties.suffix.as_ref(),
        );
        let params = match variant.fields {
            Fields::Unit => quote! {},
            Fields::Unnamed(..) => quote! { (..) },
            Fields::Named(..) => quote! { {..} },
        };

        arms.push(quote! { #name::#ident #params => #output });
    }

    if arms.len() < variants.len() {
        arms.push(quote! {
            _ => panic!(
                "AsRef::<str>::as_ref() or AsStaticRef::<str>::as_static() \
                 called on disabled variant.",
            )
        });
    }

    Ok(arms)
}

pub fn as_ref_str_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let arms = get_arms(ast, |tok| {
        quote! { ::core::convert::AsRef::<str>::as_ref(#tok) }
    })?;

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::core::convert::AsRef<str> for #name #ty_generics #where_clause {
            #[inline]
            fn as_ref(&self) -> &str {
                match *self {
                    #(#arms),*
                }
            }
        }
    })
}

pub enum GenerateTraitVariant {
    AsStaticStr,
    From,
}

pub fn as_static_str_inner(
    ast: &DeriveInput,
    trait_variant: &GenerateTraitVariant,
) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let arms = &get_arms(ast, |tok| {
        quote! { ::core::convert::From::from(#tok) }
    })?;

    let type_properties = ast.get_type_properties()?;
    let strum_module_path = type_properties.crate_module_path();

    let mut generics = ast.generics.clone();
    generics
        .params
        .push(syn::GenericParam::Lifetime(syn::LifetimeParam::new(
            parse_quote!('_derivative_strum),
        )));
    let (impl_generics2, _, _) = generics.split_for_impl();

    Ok(match trait_variant {
        GenerateTraitVariant::AsStaticStr => quote! {
            #[automatically_derived]
            impl #impl_generics #strum_module_path::AsStaticRef<str> for #name #ty_generics #where_clause {
                #[inline]
                fn as_static(&self) -> &'static str {
                    match *self {
                        #(#arms),*
                    }
                }
            }
        },
        GenerateTraitVariant::From if !type_properties.const_into_str => quote! {
            #[automatically_derived]
            impl #impl_generics ::core::convert::From<#name #ty_generics> for &'static str #where_clause {
                #[inline]
                fn from(x: #name #ty_generics) -> &'static str {
                    match x {
                        #(#arms),*
                    }
                }
            }
            #[automatically_derived]
            impl #impl_generics2 ::core::convert::From<&'_derivative_strum #name #ty_generics> for &'static str #where_clause {
                #[inline]
                fn from(x: &'_derivative_strum #name #ty_generics) -> &'static str {
                    match *x {
                        #(#arms),*
                    }
                }
            }
        },
        GenerateTraitVariant::From => quote! {
            #[automatically_derived]
            impl #impl_generics #name #ty_generics #where_clause {
                pub const fn into_str(&self) -> &'static str {
                    match self {
                        #(#arms),*
                    }
                }
            }
            #[automatically_derived]
            impl #impl_generics ::core::convert::From<#name #ty_generics> for &'static str #where_clause {
                fn from(x: #name #ty_generics) -> &'static str {
                    match x {
                        #(#arms),*
                    }
                }
            }
            #[automatically_derived]
            impl #impl_generics2 ::core::convert::From<&'_derivative_strum #name #ty_generics> for &'static str #where_clause {
                fn from(x: &'_derivative_strum #name #ty_generics) -> &'static str {
                    x.into_str()
                }
            }
        },
    })
}
