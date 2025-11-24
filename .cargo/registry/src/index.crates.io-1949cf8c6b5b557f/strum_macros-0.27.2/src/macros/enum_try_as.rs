use crate::helpers::{case_style::snakify, non_enum_error, HasStrumVariantProperties};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{Data, DeriveInput};

pub fn enum_try_as_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => return Err(non_enum_error()),
    };

    let enum_name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let variants: Vec<_> = variants
        .iter()
        .filter_map(|variant| {
            if variant.get_variant_properties().ok()?.disabled.is_some() {
                return None;
            }

            match &variant.fields {
                syn::Fields::Unnamed(values) => {
                    let variant_name = &variant.ident;
                    let types: Vec<_> = values.unnamed.iter().map(|field| {
                        field.ty.to_token_stream()
                    }).collect();
                    let field_names: Vec<_> = values.unnamed.iter().enumerate().map(|(i, _)| {
                        let name = "x".repeat(i + 1);
                        let name = format_ident!("{}", name);
                        quote! {#name}
                    }).collect();

                    let move_fn_name = format_ident!("try_as_{}", snakify(&variant_name.to_string()));
                    let ref_fn_name = format_ident!("try_as_{}_ref", snakify(&variant_name.to_string()));
                    let mut_fn_name = format_ident!("try_as_{}_mut", snakify(&variant_name.to_string()));

                    Some(quote! {
                        #[must_use]
                        #[inline]
                        pub fn #move_fn_name(self) -> ::core::option::Option<(#(#types),*)> {
                            match self {
                                #enum_name::#variant_name (#(#field_names),*) => Some((#(#field_names),*)),
                                _ => None
                            }
                        }

                        #[must_use]
                        #[inline]
                        pub const fn #ref_fn_name(&self) -> ::core::option::Option<(#(&#types),*)> {
                            match self {
                                #enum_name::#variant_name (#(#field_names),*) => Some((#(#field_names),*)),
                                _ => None
                            }
                        }

                        #[must_use]
                        #[inline]
                        pub fn #mut_fn_name(&mut self) -> ::core::option::Option<(#(&mut #types),*)> {
                            match self {
                                #enum_name::#variant_name (#(#field_names),*) => Some((#(#field_names),*)),
                                _ => None
                            }
                        }
                    })
                },
                _ => {
                    return None;
                }
            }

        })
        .collect();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #enum_name #ty_generics #where_clause {
            #(#variants)*
        }
    })
}
