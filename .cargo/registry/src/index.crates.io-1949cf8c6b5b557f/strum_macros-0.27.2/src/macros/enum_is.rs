use crate::helpers::{case_style::snakify, non_enum_error, HasStrumVariantProperties};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput};

pub fn enum_is_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => return Err(non_enum_error()),
    };
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let enum_name = &ast.ident;
    let variants: Vec<_> = variants
        .iter()
        .filter_map(|variant| {
            if variant.get_variant_properties().ok()?.disabled.is_some() {
                return None;
            }

            let variant_name = &variant.ident;
            let fn_name = format_ident!("is_{}", snakify(&variant_name.to_string()));
            let doc_comment = format!(
                "Returns [true] if the enum is [{}::{}] otherwise [false]",
                enum_name, variant_name
            );
            Some(quote! {
                #[must_use]
                #[inline]
                #[doc = #doc_comment]
                pub const fn #fn_name(&self) -> bool {
                    match self {
                        &#enum_name::#variant_name { .. } => true,
                        _ => false
                    }
                }
            })
        })
        .collect();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #enum_name  #ty_generics #where_clause {
            #(#variants)*
        }
    }
    .into())
}
