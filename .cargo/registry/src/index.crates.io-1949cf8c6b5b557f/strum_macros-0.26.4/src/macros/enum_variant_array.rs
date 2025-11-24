use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

use crate::helpers::{non_enum_error, non_unit_variant_error, HasTypeProperties};

pub fn static_variants_array_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let gen = &ast.generics;
    let (impl_generics, ty_generics, where_clause) = gen.split_for_impl();

    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => return Err(non_enum_error()),
    };

    let type_properties = ast.get_type_properties()?;
    let strum_module_path = type_properties.crate_module_path();

    let idents = variants
        .iter()
        .cloned()
        .map(|v| match v.fields {
            Fields::Unit => Ok(v.ident),
            _ => Err(non_unit_variant_error()),
        })
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        impl #impl_generics #strum_module_path::VariantArray for #name #ty_generics #where_clause {
            const VARIANTS: &'static [Self] = &[ #(#name::#idents),* ];
        }
    })
}
