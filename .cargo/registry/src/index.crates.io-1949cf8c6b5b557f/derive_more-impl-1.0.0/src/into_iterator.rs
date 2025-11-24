use crate::utils::{
    add_extra_generic_param, add_extra_where_clauses, SingleFieldData, State,
};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse::Result, DeriveInput};

/// Provides the hook to expand `#[derive(IntoIterator)]` into an implementation of `IntoIterator`
pub fn expand(input: &DeriveInput, trait_name: &'static str) -> Result<TokenStream> {
    let state =
        State::with_field_ignore_and_refs(input, trait_name, "into_iterator".into())?;
    let SingleFieldData {
        input_type,
        info,
        field_type,
        member,
        trait_path,
        ..
    } = state.assert_single_enabled_field();

    let mut tokens = TokenStream::new();

    for ref_type in info.ref_types() {
        let reference = ref_type.reference();
        let lifetime = ref_type.lifetime();
        let reference_with_lifetime = ref_type.reference_with_lifetime();

        let generics_impl;
        let (impl_generics, _, _) = if ref_type.is_ref() {
            generics_impl = add_extra_generic_param(&input.generics, lifetime.clone());
            generics_impl.split_for_impl()
        } else {
            input.generics.split_for_impl()
        };

        let generics = add_extra_where_clauses(
            &input.generics,
            quote! { where #reference_with_lifetime #field_type: #trait_path },
        );
        let (_, ty_generics, where_clause) = generics.split_for_impl();

        let casted_trait = &quote! {
            <#reference_with_lifetime #field_type as #trait_path>
        };
        let into_iterator = quote! {
            #[automatically_derived]
            impl #impl_generics #trait_path for #reference_with_lifetime #input_type #ty_generics
                 #where_clause
            {
                type Item = #casted_trait::Item;
                type IntoIter = #casted_trait::IntoIter;

                #[inline]
                fn into_iter(self) -> Self::IntoIter {
                    #casted_trait::into_iter(#reference #member)
                }
            }
        };
        into_iterator.to_tokens(&mut tokens);
    }
    Ok(tokens)
}
