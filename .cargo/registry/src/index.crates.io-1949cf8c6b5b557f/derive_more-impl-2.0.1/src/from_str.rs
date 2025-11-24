use crate::utils::{DeriveType, HashMap};
use crate::utils::{SingleFieldData, State};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::Result, DeriveInput};

/// Provides the hook to expand `#[derive(FromStr)]` into an implementation of `FromStr`
pub fn expand(input: &DeriveInput, trait_name: &'static str) -> Result<TokenStream> {
    let state = State::new(input, trait_name, trait_name.to_lowercase())?;

    if state.derive_type == DeriveType::Enum {
        Ok(enum_from(input, state, trait_name))
    } else {
        Ok(struct_from(&state, trait_name))
    }
}

pub fn struct_from(state: &State, trait_name: &'static str) -> TokenStream {
    // We cannot set defaults for fields, once we do we can remove this check
    if state.fields.len() != 1 || state.enabled_fields().len() != 1 {
        panic_one_field(trait_name);
    }

    let single_field_data = state.assert_single_enabled_field();
    let SingleFieldData {
        input_type,
        field_type,
        trait_path,
        casted_trait,
        impl_generics,
        ty_generics,
        where_clause,
        ..
    } = single_field_data.clone();

    let initializers = [quote! { #casted_trait::from_str(src)? }];
    let body = single_field_data.initializer(&initializers);
    let error = quote! { <#field_type as #trait_path>::Err };

    quote! {
        #[automatically_derived]
        impl #impl_generics #trait_path for #input_type #ty_generics #where_clause {
            type Err = #error;

            #[inline]
            fn from_str(src: &str) -> derive_more::core::result::Result<Self, #error> {
                derive_more::core::result::Result::Ok(#body)
            }
        }
    }
}

fn enum_from(
    input: &DeriveInput,
    state: State,
    trait_name: &'static str,
) -> TokenStream {
    let mut variants_caseinsensitive = HashMap::default();
    for variant_state in state.enabled_variant_data().variant_states {
        let variant = variant_state.variant.unwrap();
        if !variant.fields.is_empty() {
            panic!("Only enums with no fields can derive({trait_name})")
        }

        variants_caseinsensitive
            .entry(variant.ident.to_string().to_lowercase())
            .or_insert_with(Vec::new)
            .push(variant.ident.clone());
    }

    let input_type = &input.ident;
    let input_type_name = input_type.to_string();

    let mut cases = vec![];

    // if a case insensitive match is unique match do that
    // otherwise do a case sensitive match
    for (ref canonical, ref variants) in variants_caseinsensitive {
        if variants.len() == 1 {
            let variant = &variants[0];
            cases.push(quote! {
                #canonical => #input_type::#variant,
            })
        } else {
            for variant in variants {
                let variant_str = variant.to_string();
                cases.push(quote! {
                    #canonical if(src == #variant_str) => #input_type::#variant,
                })
            }
        }
    }

    let trait_path = state.trait_path;

    quote! {
        impl #trait_path for #input_type {
            type Err = derive_more::FromStrError;

            #[inline]
            fn from_str(src: &str) -> derive_more::core::result::Result<Self, derive_more::FromStrError> {
                Ok(match src.to_lowercase().as_str() {
                    #(#cases)*
                    _ => return Err(derive_more::FromStrError::new(#input_type_name)),
                })
            }
        }
    }
}

fn panic_one_field(trait_name: &str) -> ! {
    panic!("Only structs with one field can derive({trait_name})")
}
