use crate::add_assign_like;
use crate::mul_helpers::generics_and_exprs;
use crate::utils::{AttrParams, HashSet, MultiFieldData, RefType, State};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::iter;
use syn::{DeriveInput, Result};

pub fn expand(input: &DeriveInput, trait_name: &'static str) -> Result<TokenStream> {
    let method_name = trait_name
        .to_lowercase()
        .trim_end_matches("assign")
        .to_string()
        + "_assign";

    let mut state = State::with_attr_params(
        input,
        trait_name,
        method_name,
        AttrParams::struct_(vec!["forward"]),
    )?;
    if state.default_info.forward {
        return Ok(add_assign_like::expand(input, trait_name));
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
    let trait_path_iter = iter::repeat(trait_path_with_params);

    let type_where_clauses = quote! {
        where #(#tys: #trait_path_iter),*
    };

    let (generics, exprs) = generics_and_exprs(
        multi_field_data.clone(),
        &scalar_ident,
        type_where_clauses,
        RefType::Mut,
    );
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #trait_path<#scalar_ident> for #input_type #ty_generics #where_clause {
            #[inline]
            #[track_caller]
            fn #method_ident(&mut self, rhs: #scalar_ident) {
                #( #exprs; )*
            }
        }
    })
}
