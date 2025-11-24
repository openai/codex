use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::helpers::{
    non_enum_error, occurrence_error, HasInnerVariantProperties, HasStrumVariantProperties,
    HasTypeProperties,
};

pub fn from_string_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => return Err(non_enum_error()),
    };

    let type_properties = ast.get_type_properties()?;
    let strum_module_path = type_properties.crate_module_path();

    let mut default_kw = None;
    let mut default =
        quote! { ::core::result::Result::Err(#strum_module_path::ParseError::VariantNotFound) };

    let mut phf_exact_match_arms = Vec::new();
    let mut standard_match_arms = Vec::new();
    for variant in variants {
        let ident = &variant.ident;
        let variant_properties = variant.get_variant_properties()?;

        if variant_properties.disabled.is_some() {
            continue;
        }

        if let Some(kw) = variant_properties.default {
            if let Some(fst_kw) = default_kw {
                return Err(occurrence_error(fst_kw, kw, "default"));
            }

            match &variant.fields {
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {}
                _ => {
                    return Err(syn::Error::new_spanned(
                        variant,
                        "Default only works on newtype structs with a single String field",
                    ))
                }
            }
            default_kw = Some(kw);
            default = quote! {
                ::core::result::Result::Ok(#name::#ident(s.into()))
            };
            continue;
        }

        let params = match &variant.fields {
            Fields::Unit => quote! {},
            Fields::Unnamed(fields) => {
                if let Some(ref value) = variant_properties.default_with {
                    let func = proc_macro2::Ident::new(&value.value(), value.span());
                    let defaults = vec![quote! { #func() }];
                    quote! { (#(#defaults),*) }
                } else {
                    let defaults =
                        ::core::iter::repeat(quote!(Default::default())).take(fields.unnamed.len());
                    quote! { (#(#defaults),*) }
                }
            }
            Fields::Named(fields) => {
                let mut defaults = vec![];
                for field in &fields.named {
                    let meta = field.get_variant_inner_properties()?;
                    let field = field.ident.as_ref().unwrap();

                    if let Some(default_with) = meta.default_with {
                        let func =
                            proc_macro2::Ident::new(&default_with.value(), default_with.span());
                        defaults.push(quote! {
                            #field: #func()
                        });
                    } else {
                        defaults.push(quote! { #field: Default::default() });
                    }
                }

                quote! { {#(#defaults),*} }
            }
        };

        let is_ascii_case_insensitive = variant_properties
            .ascii_case_insensitive
            .unwrap_or(type_properties.ascii_case_insensitive);

        // If we don't have any custom variants, add the default serialized name.
        for serialization in variant_properties.get_serializations(type_properties.case_style) {
            if type_properties.use_phf {
                phf_exact_match_arms.push(quote! { #serialization => #name::#ident #params, });

                if is_ascii_case_insensitive {
                    // Store the lowercase and UPPERCASE variants in the phf map to capture
                    let ser_string = serialization.value();

                    let lower =
                        syn::LitStr::new(&ser_string.to_ascii_lowercase(), serialization.span());
                    let upper =
                        syn::LitStr::new(&ser_string.to_ascii_uppercase(), serialization.span());
                    phf_exact_match_arms.push(quote! { #lower => #name::#ident #params, });
                    phf_exact_match_arms.push(quote! { #upper => #name::#ident #params, });
                    standard_match_arms.push(quote! { s if s.eq_ignore_ascii_case(#serialization) => #name::#ident #params, });
                }
            } else {
                standard_match_arms.push(if !is_ascii_case_insensitive {
                    quote! { #serialization => #name::#ident #params, }
                } else {
                    quote! { s if s.eq_ignore_ascii_case(#serialization) => #name::#ident #params, }
                });
            }
        }
    }

    let phf_body = if phf_exact_match_arms.is_empty() {
        quote!()
    } else {
        quote! {
            use #strum_module_path::_private_phf_reexport_for_macro_if_phf_feature as phf;
            static PHF: phf::Map<&'static str, #name> = phf::phf_map! {
                #(#phf_exact_match_arms)*
            };
            if let Some(value) = PHF.get(s).cloned() {
                return ::core::result::Result::Ok(value);
            }
        }
    };

    let standard_match_body = if standard_match_arms.is_empty() {
        default
    } else {
        quote! {
            ::core::result::Result::Ok(match s {
                #(#standard_match_arms)*
                _ => return #default,
            })
        }
    };

    let from_str = quote! {
        #[allow(clippy::use_self)]
        impl #impl_generics ::core::str::FromStr for #name #ty_generics #where_clause {
            type Err = #strum_module_path::ParseError;
            fn from_str(s: &str) -> ::core::result::Result< #name #ty_generics , <Self as ::core::str::FromStr>::Err> {
                #phf_body
                #standard_match_body
            }
        }
    };
    let try_from_str = try_from_str(
        name,
        &impl_generics,
        &ty_generics,
        where_clause,
        &strum_module_path,
    );

    Ok(quote! {
        #from_str
        #try_from_str
    })
}

#[rustversion::before(1.34)]
fn try_from_str(
    _name: &proc_macro2::Ident,
    _impl_generics: &syn::ImplGenerics,
    _ty_generics: &syn::TypeGenerics,
    _where_clause: Option<&syn::WhereClause>,
    _strum_module_path: &syn::Path,
) -> TokenStream {
    Default::default()
}

#[rustversion::since(1.34)]
fn try_from_str(
    name: &proc_macro2::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    strum_module_path: &syn::Path,
) -> TokenStream {
    quote! {
        #[allow(clippy::use_self)]
        impl #impl_generics ::core::convert::TryFrom<&str> for #name #ty_generics #where_clause {
            type Error = #strum_module_path::ParseError;
            fn try_from(s: &str) -> ::core::result::Result< #name #ty_generics , <Self as ::core::convert::TryFrom<&str>>::Error> {
                ::core::str::FromStr::from_str(s)
            }
        }
    }
}
