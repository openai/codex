use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Type};

use crate::helpers::{non_enum_error, HasStrumVariantProperties, HasTypeProperties};

pub fn from_repr_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let gen = &ast.generics;
    let (impl_generics, ty_generics, where_clause) = gen.split_for_impl();
    let vis = &ast.vis;

    let mut discriminant_type: Type = syn::parse("usize".parse().unwrap()).unwrap();
    if let Some(type_path) = ast
        .get_type_properties()
        .ok()
        .and_then(|tp| tp.enum_repr)
        .and_then(|repr_ts| syn::parse2::<Type>(repr_ts).ok())
    {
        if let Type::Path(path) = type_path.clone() {
            if let Some(seg) = path.path.segments.last() {
                for t in &[
                    "u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize",
                ] {
                    if seg.ident == t {
                        discriminant_type = type_path;
                        break;
                    }
                }
            }
        }
    }

    if gen.lifetimes().count() > 0 {
        return Err(syn::Error::new(
            Span::call_site(),
            "This macro doesn't support enums with lifetimes. \
             The resulting enums would be unbounded.",
        ));
    }

    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => return Err(non_enum_error()),
    };

    let mut arms = Vec::new();
    let mut constant_defs = Vec::new();
    let mut has_additional_data = false;
    let mut prev_const_var_ident = None;
    for variant in variants {
        if variant.get_variant_properties()?.disabled.is_some() {
            continue;
        }

        let ident = &variant.ident;
        let params = match &variant.fields {
            Fields::Unit => quote! {},
            Fields::Unnamed(fields) => {
                has_additional_data = true;
                let defaults = ::core::iter::repeat(quote!(::core::default::Default::default()))
                    .take(fields.unnamed.len());
                quote! { (#(#defaults),*) }
            }
            Fields::Named(fields) => {
                has_additional_data = true;
                let fields = fields
                    .named
                    .iter()
                    .map(|field| field.ident.as_ref().unwrap());
                quote! { {#(#fields: ::core::default::Default::default()),*} }
            }
        };

        let const_var_str = format!("{}_DISCRIMINANT", variant.ident);
        let const_var_ident = format_ident!("{}", const_var_str);

        let const_val_expr = match &variant.discriminant {
            Some((_, expr)) => quote! { #expr },
            None => match &prev_const_var_ident {
                Some(prev) => quote! { #prev + 1 },
                None => quote! { 0 },
            },
        };

        constant_defs.push(quote! {
            #[allow(non_upper_case_globals)]
            const #const_var_ident: #discriminant_type = #const_val_expr;
        });
        arms.push(quote! {v if v == #const_var_ident => ::core::option::Option::Some(#name::#ident #params)});

        prev_const_var_ident = Some(const_var_ident);
    }

    arms.push(quote! { _ => ::core::option::Option::None });

    let const_if_possible = if has_additional_data {
        quote! {}
    } else {
        #[rustversion::before(1.46)]
        fn filter_by_rust_version(_: TokenStream) -> TokenStream {
            quote! {}
        }

        #[rustversion::since(1.46)]
        fn filter_by_rust_version(s: TokenStream) -> TokenStream {
            s
        }
        filter_by_rust_version(quote! { const })
    };

    Ok(quote! {
        #[allow(clippy::use_self)]
        impl #impl_generics #name #ty_generics #where_clause {
            #[doc = "Try to create [Self] from the raw representation"]
            #vis #const_if_possible fn from_repr(discriminant: #discriminant_type) -> Option<#name #ty_generics> {
                #(#constant_defs)*
                match discriminant {
                    #(#arms),*
                }
            }
        }
    })
}
