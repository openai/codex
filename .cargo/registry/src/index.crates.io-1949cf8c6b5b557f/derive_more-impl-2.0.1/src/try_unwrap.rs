use crate::utils::{AttrParams, DeriveType, State};
use convert_case::{Case, Casing};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Fields, Ident, Result, Type};

pub fn expand(input: &DeriveInput, trait_name: &'static str) -> Result<TokenStream> {
    let state = State::with_attr_params(
        input,
        trait_name,
        "try_unwrap".into(),
        AttrParams {
            enum_: vec!["ignore", "owned", "ref", "ref_mut"],
            variant: vec!["ignore", "owned", "ref", "ref_mut"],
            struct_: vec!["ignore"],
            field: vec!["ignore"],
        },
    )?;
    assert!(
        state.derive_type == DeriveType::Enum,
        "TryUnwrap can only be derived for enums",
    );

    let enum_name = &input.ident;
    let (imp_generics, type_generics, where_clause) = input.generics.split_for_impl();

    let variant_data = state.enabled_variant_data();

    let mut funcs = vec![];
    for (variant_state, info) in
        Iterator::zip(variant_data.variant_states.iter(), variant_data.infos)
    {
        let variant = variant_state.variant.unwrap();
        let fn_name = format_ident!(
            "try_unwrap_{ident}",
            ident = variant.ident.to_string().to_case(Case::Snake),
            span = variant.ident.span(),
        );
        let ref_fn_name = format_ident!(
            "try_unwrap_{ident}_ref",
            ident = variant.ident.to_string().to_case(Case::Snake),
            span = variant.ident.span(),
        );
        let mut_fn_name = format_ident!(
            "try_unwrap_{ident}_mut",
            ident = variant.ident.to_string().to_case(Case::Snake),
            span = variant.ident.span(),
        );
        let variant_ident = &variant.ident;
        let (data_pattern, ret_value, data_types) = get_field_info(&variant.fields);
        let pattern = quote! { #enum_name :: #variant_ident #data_pattern };

        let (failed_block, failed_block_ref, failed_block_mut) = (
            failed_block(&state, enum_name, &fn_name),
            failed_block(&state, enum_name, &ref_fn_name),
            failed_block(&state, enum_name, &mut_fn_name),
        );

        let doc_owned = format!(
            "Attempts to unwrap this value to the `{enum_name}::{variant_ident}` variant.\n",
        );
        let doc_ref = format!(
            "Attempts to unwrap this reference to the `{enum_name}::{variant_ident}` variant.\n",
        );
        let doc_mut = format!(
            "Attempts to unwrap this mutable reference to the `{enum_name}::{variant_ident}` variant.\n",
        );
        let doc_else = "Returns a [TryUnwrapError] with the original value if this value is of any other type.";
        let func = quote! {
            #[inline]
            #[track_caller]
            #[doc = #doc_owned]
            #[doc = #doc_else]
            pub fn #fn_name(self) -> derive_more::core::result::Result<
                (#(#data_types),*), derive_more::TryUnwrapError<Self>
            > {
                match self {
                    #pattern => derive_more::core::result::Result::Ok(#ret_value),
                    val @ _ => #failed_block,
                }
            }
        };

        let ref_func = quote! {
            #[inline]
            #[track_caller]
            #[doc = #doc_ref]
            #[doc = #doc_else]
            pub fn #ref_fn_name(&self) -> derive_more::core::result::Result<
                (#(&#data_types),*), derive_more::TryUnwrapError<&Self>
            > {
                match self {
                    #pattern => derive_more::core::result::Result::Ok(#ret_value),
                    val @ _ => #failed_block_ref,
                }
            }
        };

        let mut_func = quote! {
            #[inline]
            #[track_caller]
            #[doc = #doc_mut]
            #[doc = #doc_else]
            pub fn #mut_fn_name(&mut self) -> derive_more::core::result::Result<
                (#(&mut #data_types),*), derive_more::TryUnwrapError<&mut Self>
            > {
                match self {
                    #pattern => derive_more::core::result::Result::Ok(#ret_value),
                    val @ _ => #failed_block_mut,
                }
            }
        };

        if info.owned && state.default_info.owned {
            funcs.push(func);
        }
        if info.ref_ && state.default_info.ref_ {
            funcs.push(ref_func);
        }
        if info.ref_mut && state.default_info.ref_mut {
            funcs.push(mut_func);
        }
    }

    let imp = quote! {
        #[allow(unreachable_code)] // omit warnings for `!` and other unreachable types
        #[automatically_derived]
        impl #imp_generics #enum_name #type_generics #where_clause {
            #(#funcs)*
        }
    };

    Ok(imp)
}

fn get_field_info(fields: &Fields) -> (TokenStream, TokenStream, Vec<&Type>) {
    match fields {
        Fields::Named(_) => panic!("cannot unwrap anonymous records"),
        Fields::Unnamed(ref fields) => {
            let (idents, types) = fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(n, it)| (format_ident!("field_{n}"), &it.ty))
                .unzip::<_, _, Vec<_>, Vec<_>>();
            (quote! { (#(#idents),*) }, quote! { (#(#idents),*) }, types)
        }
        Fields::Unit => (quote! {}, quote! { () }, vec![]),
    }
}

fn failed_block(state: &State, enum_name: &Ident, func_name: &Ident) -> TokenStream {
    let arms = state
        .variant_states
        .iter()
        .map(|it| it.variant.unwrap())
        .map(|variant| {
            let data_pattern = match variant.fields {
                Fields::Named(_) => quote! { {..} },
                Fields::Unnamed(_) => quote! { (..) },
                Fields::Unit => quote! {},
            };
            let variant_ident = &variant.ident;
            let error = quote! {
                derive_more::TryUnwrapError::<_>::new(
                    val,
                    stringify!(#enum_name),
                    stringify!(#variant_ident),
                    stringify!(#func_name),
                )
            };
            quote! {
                val @ #enum_name :: #variant_ident #data_pattern
                    => derive_more::core::result::Result::Err(#error)
            }
        });

    quote! {
        match val {
            #(#arms),*
        }
    }
}
