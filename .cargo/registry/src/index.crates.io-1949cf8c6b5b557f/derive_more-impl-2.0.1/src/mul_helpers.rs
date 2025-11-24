use crate::utils::{add_where_clauses_for_new_ident, MultiFieldData, RefType};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Generics, Ident};

pub fn generics_and_exprs(
    multi_field_data: MultiFieldData,
    scalar_ident: &Ident,
    type_where_clauses: TokenStream,
    ref_type: RefType,
) -> (Generics, Vec<TokenStream>) {
    let MultiFieldData {
        fields,
        casted_traits,
        members,
        method_ident,
        ..
    } = multi_field_data;
    let reference = ref_type.reference();
    let exprs: Vec<_> = casted_traits
        .iter()
        .zip(members)
        .map(|(casted_trait, member)| {
            quote! { #casted_trait::#method_ident(#reference #member, rhs) }
        })
        .collect();

    let new_generics = add_where_clauses_for_new_ident(
        &multi_field_data.state.input.generics,
        &fields,
        scalar_ident,
        type_where_clauses,
        true,
    );
    (new_generics, exprs)
}
