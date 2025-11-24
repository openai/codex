use crate::utils::{
    add_extra_generic_param, numbered_vars, AttrParams, DeriveType, MultiFieldData,
    State,
};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{DeriveInput, Result};

use crate::utils::HashMap;

/// Provides the hook to expand `#[derive(TryInto)]` into an implementation of `TryInto`
#[allow(clippy::cognitive_complexity)]
pub fn expand(input: &DeriveInput, trait_name: &'static str) -> Result<TokenStream> {
    let state = State::with_attr_params(
        input,
        trait_name,
        "try_into".into(),
        AttrParams {
            enum_: vec!["ignore", "owned", "ref", "ref_mut"],
            variant: vec!["ignore", "owned", "ref", "ref_mut"],
            struct_: vec!["ignore", "owned", "ref", "ref_mut"],
            field: vec!["ignore"],
        },
    )?;
    assert!(
        state.derive_type == DeriveType::Enum,
        "Only enums can derive TryInto"
    );

    let mut variants_per_types = HashMap::default();

    for variant_state in state.enabled_variant_data().variant_states {
        let multi_field_data = variant_state.enabled_fields_data();
        let MultiFieldData {
            variant_info,
            field_types,
            ..
        } = multi_field_data.clone();
        for ref_type in variant_info.ref_types() {
            variants_per_types
                .entry((ref_type, field_types.clone()))
                .or_insert_with(Vec::new)
                .push(multi_field_data.clone());
        }
    }

    let mut tokens = TokenStream::new();

    for ((ref_type, ref original_types), ref multi_field_data) in variants_per_types {
        let input_type = &input.ident;

        let pattern_ref = ref_type.pattern_ref();
        let lifetime = ref_type.lifetime();
        let reference_with_lifetime = ref_type.reference_with_lifetime();

        let mut matchers = vec![];
        let vars = &numbered_vars(original_types.len(), "");
        for multi_field_data in multi_field_data {
            let patterns: Vec<_> = vars
                .iter()
                .map(|var| quote! { #pattern_ref #var })
                .collect();
            matchers.push(
                multi_field_data.matcher(&multi_field_data.field_indexes, &patterns),
            );
        }

        let vars = if vars.len() == 1 {
            quote! { #(#vars)* }
        } else {
            quote! { (#(#vars),*) }
        };

        let output_type = if original_types.len() == 1 {
            quote! { #(#original_types)* }.to_string()
        } else {
            let types = original_types
                .iter()
                .map(|t| quote! { #t }.to_string())
                .collect::<Vec<_>>();
            format!("({})", types.join(", "))
        };
        let variant_names = multi_field_data
            .iter()
            .map(|d| {
                d.variant_name
                    .expect("Somehow there was no variant name")
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(", ");

        let generics_impl;
        let (_, ty_generics, where_clause) = input.generics.split_for_impl();
        let (impl_generics, _, _) = if ref_type.is_ref() {
            generics_impl = add_extra_generic_param(&input.generics, lifetime.clone());
            generics_impl.split_for_impl()
        } else {
            input.generics.split_for_impl()
        };

        let error = quote! {
            derive_more::TryIntoError<#reference_with_lifetime #input_type #ty_generics>
        };

        let try_from = quote! {
            #[automatically_derived]
            impl #impl_generics derive_more::core::convert::TryFrom<
                #reference_with_lifetime #input_type #ty_generics
            > for (#(#reference_with_lifetime #original_types),*) #where_clause {
                type Error = #error;

                #[inline]
                fn try_from(
                    value: #reference_with_lifetime #input_type #ty_generics,
                ) -> derive_more::core::result::Result<Self, #error> {
                    match value {
                        #(#matchers)|* => derive_more::core::result::Result::Ok(#vars),
                        _ => derive_more::core::result::Result::Err(
                            derive_more::TryIntoError::new(value, #variant_names, #output_type),
                        ),
                    }
                }
            }
        };
        try_from.to_tokens(&mut tokens)
    }
    Ok(tokens)
}
