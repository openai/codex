use crate::utils::{add_where_clauses_for_new_ident, SingleFieldData, State};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse::Result, DeriveInput};

/// Provides the hook to expand `#[derive(Index)]` into an implementation of `Index`
pub fn expand(input: &DeriveInput, trait_name: &'static str) -> Result<TokenStream> {
    let index_type = format_ident!("__IdxT");
    let mut state =
        State::with_field_ignore(input, trait_name, trait_name.to_lowercase())?;
    state.add_trait_path_type_param(quote! { #index_type });
    let SingleFieldData {
        field,
        field_type,
        input_type,
        trait_path_with_params,
        casted_trait,
        member,
        ..
    } = state.assert_single_enabled_field();

    let type_where_clauses = quote! {
        where #field_type: #trait_path_with_params
    };

    let new_generics = add_where_clauses_for_new_ident(
        &input.generics,
        &[field],
        &index_type,
        type_where_clauses,
        true,
    );

    let (impl_generics, _, where_clause) = new_generics.split_for_impl();
    let (_, ty_generics, _) = input.generics.split_for_impl();
    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #trait_path_with_params for #input_type #ty_generics #where_clause {
            type Output = #casted_trait::Output;

            #[inline]
            fn index(&self, idx: #index_type) -> &Self::Output {
                #casted_trait::index(&#member, idx)
            }
        }
    })
}
