use crate::utils::{add_extra_where_clauses, SingleFieldData, State};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::Result, DeriveInput};

/// Provides the hook to expand `#[derive(DerefMut)]` into an implementation of `DerefMut`
pub fn expand(input: &DeriveInput, trait_name: &'static str) -> Result<TokenStream> {
    let state =
        State::with_field_ignore_and_forward(input, trait_name, "deref_mut".into())?;
    let SingleFieldData {
        input_type,
        trait_path,
        casted_trait,
        ty_generics,
        field_type,
        member,
        info,
        ..
    } = state.assert_single_enabled_field();
    let (body, generics) = if info.forward {
        (
            quote! { #casted_trait::deref_mut(&mut #member) },
            add_extra_where_clauses(
                &input.generics,
                quote! {
                    where #field_type: #trait_path
                },
            ),
        )
    } else {
        (quote! { &mut #member }, input.generics.clone())
    };
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #trait_path for #input_type #ty_generics #where_clause {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                #body
            }
        }
    })
}
