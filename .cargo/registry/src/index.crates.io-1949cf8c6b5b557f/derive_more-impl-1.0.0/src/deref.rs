use crate::utils::{add_extra_where_clauses, SingleFieldData, State};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::Result, DeriveInput};

/// Provides the hook to expand `#[derive(Deref)]` into an implementation of `Deref`
pub fn expand(input: &DeriveInput, trait_name: &'static str) -> Result<TokenStream> {
    let state = State::with_field_ignore_and_forward(
        input,
        trait_name,
        trait_name.to_lowercase(),
    )?;
    let SingleFieldData {
        input_type,
        field_type,
        trait_path,
        casted_trait,
        ty_generics,
        member,
        info,
        ..
    } = state.assert_single_enabled_field();

    let (target, body, generics) = if info.forward {
        (
            quote! { #casted_trait::Target },
            quote! { #casted_trait::deref(&#member) },
            add_extra_where_clauses(
                &input.generics,
                quote! {
                    where #field_type: #trait_path
                },
            ),
        )
    } else {
        (
            quote! { #field_type },
            quote! { &#member },
            input.generics.clone(),
        )
    };
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics #trait_path for #input_type #ty_generics #where_clause {
            type Target = #target;

            #[inline]
            fn deref(&self) -> &Self::Target {
                #body
            }
        }
    })
}
