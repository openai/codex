use crate::add_like;
use crate::mul_helpers::generics_and_exprs;
use crate::utils::{AttrParams, HashSet, MultiFieldData, RefType, State};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::iter;
use syn::{DeriveInput, Result};

pub fn expand(input: &DeriveInput, trait_name: &'static str) -> Result<TokenStream> {
    let mut state = State::with_attr_params(
        input,
        trait_name,
        trait_name.to_lowercase(),
        AttrParams::struct_(vec!["forward"]),
    )?;
    if state.default_info.forward {
        return Ok(add_like::expand(input, trait_name));
    }

    let scalar_ident = format_ident!("__RhsT");
    state.add_trait_path_type_param(quote! { #scalar_ident });
    let multi_field_data = state.enabled_fields_data();
    let MultiFieldData {
        input_type,
        field_types,
        ty_generics,
        trait_path,
        trait_path_with_params,
        method_ident,
        ..
    } = multi_field_data.clone();

    let tys = field_types.iter().collect::<HashSet<_>>();
    let tys = tys.iter();
    let scalar_iter = iter::repeat(&scalar_ident);
    let trait_path_iter = iter::repeat(trait_path);

    let type_where_clauses = quote! {
        where #(#tys: #trait_path_iter<#scalar_iter, Output=#tys>),*
    };

    let (generics, initializers) = generics_and_exprs(
        multi_field_data.clone(),
        &scalar_ident,
        type_where_clauses,
        RefType::No,
    );
    let body = multi_field_data.initializer(&initializers);
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics  #trait_path_with_params for #input_type #ty_generics #where_clause {
            type Output = #input_type #ty_generics;

            #[inline]
            #[track_caller]
            fn #method_ident(self, rhs: #scalar_ident) -> #input_type #ty_generics {
                #body
            }
        }
    })
}
