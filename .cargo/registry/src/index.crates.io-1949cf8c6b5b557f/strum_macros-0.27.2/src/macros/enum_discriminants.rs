use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use syn::parse_quote;
use syn::{Data, DeriveInput, Fields};

use crate::helpers::{non_enum_error, strum_discriminants_passthrough_error, HasTypeProperties};

/// Attributes to copy from the main enum's variants to the discriminant enum's variants.
///
/// Attributes not in this list may be for other `proc_macro`s on the main enum, and may cause
/// compilation problems when copied across.
const ATTRIBUTES_TO_COPY: &[&str] = &["doc", "cfg", "allow", "deny", "strum_discriminants"];

pub fn enum_discriminants_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let vis = &ast.vis;

    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => return Err(non_enum_error()),
    };

    // Derives for the generated enum
    let type_properties = ast.get_type_properties()?;
    let strum_module_path = type_properties.crate_module_path();

    let derives = type_properties.discriminant_derives;

    let derives = quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, #(#derives),*)]
    };

    // Create #[doc] attrs for new generated type.
    let docs = type_properties.discriminant_docs;

    let docs = quote! {
        #(#[doc = #docs])*
    };

    // Work out the name
    let default_name = syn::Ident::new(&format!("{}Discriminants", name), Span::call_site());

    let discriminants_name = type_properties.discriminant_name.unwrap_or(default_name);
    let discriminants_vis = type_properties
        .discriminant_vis
        .as_ref()
        .unwrap_or_else(|| &vis);

    // Pass through all other attributes
    let pass_though_attributes = type_properties.discriminant_others;

    let repr = type_properties.enum_repr.map(|repr| quote!(#[repr(#repr)]));

    // Add the variants without fields, but exclude the `strum` meta item
    let mut discriminants = Vec::new();
    for variant in variants {
        let ident = &variant.ident;
        let discriminant = variant
            .discriminant
            .as_ref()
            .map(|(_, expr)| quote!( = #expr));

        // Don't copy across the "strum" meta attribute. Only passthrough the whitelisted
        // attributes and proxy `#[strum_discriminants(...)]` attributes
        let attrs = variant
            .attrs
            .iter()
            .filter(|attr| {
                ATTRIBUTES_TO_COPY
                    .iter()
                    .any(|attr_whitelisted| attr.path().is_ident(attr_whitelisted))
            })
            .map(|attr| {
                if attr.path().is_ident("strum_discriminants") {
                    let mut ts = attr.meta.require_list()?.to_token_stream().into_iter();

                    // Discard strum_discriminants(...)
                    let _ = ts.next();

                    let passthrough_group = ts
                        .next()
                        .ok_or_else(|| strum_discriminants_passthrough_error(attr))?;
                    let passthrough_attribute = match passthrough_group {
                        TokenTree::Group(ref group) => group.stream(),
                        _ => {
                            return Err(strum_discriminants_passthrough_error(&passthrough_group));
                        }
                    };
                    if passthrough_attribute.is_empty() {
                        return Err(strum_discriminants_passthrough_error(&passthrough_group));
                    }
                    Ok(quote! { #[#passthrough_attribute] })
                } else {
                    Ok(attr.to_token_stream())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        discriminants.push(quote! { #(#attrs)* #ident #discriminant});
    }

    // Ideally:
    //
    // * For `Copy` types, we `impl From<TheEnum> for TheEnumDiscriminants`
    // * For `!Copy` types, we `impl<'enum> From<&'enum TheEnum> for TheEnumDiscriminants`
    //
    // That way we ensure users are not able to pass a `Copy` type by reference. However, the
    // `#[derive(..)]` attributes are not in the parsed tokens, so we are not able to check if a
    // type is `Copy`, so we just implement both.
    //
    // See <https://github.com/dtolnay/syn/issues/433>
    // ---
    // let is_copy = unique_meta_list(type_meta.iter(), "derive")
    //     .map(extract_list_metas)
    //     .map(|metas| {
    //         metas
    //             .filter_map(get_meta_ident)
    //             .any(|derive| derive.to_string() == "Copy")
    //     }).unwrap_or(false);

    let arms = variants
        .iter()
        .map(|variant| {
            let ident = &variant.ident;
            let params = match &variant.fields {
                Fields::Unit => quote! {},
                Fields::Unnamed(_fields) => {
                    quote! { (..) }
                }
                Fields::Named(_fields) => {
                    quote! { { .. } }
                }
            };

            quote! { #name::#ident #params => #discriminants_name::#ident }
        })
        .collect::<Vec<_>>();

    let from_fn_body = if variants.is_empty() {
        //this method on empty enum is impossible to be called. it is therefor left empty
        quote! { unreachable!()}
    } else {
        quote! { match val { #(#arms),* } }
    };

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    let impl_from = quote! {
        #[automatically_derived]
        impl #impl_generics ::core::convert::From< #name #ty_generics > for #discriminants_name #where_clause {
            #[inline]
            fn from(val: #name #ty_generics) -> #discriminants_name {
                #from_fn_body
            }
        }
    };
    let impl_from_ref = {
        let mut generics = ast.generics.clone();

        let lifetime = parse_quote!('_enum);
        let enum_life = quote! { & #lifetime };
        generics.params.push(lifetime);

        // Shadows the earlier `impl_generics`
        let (impl_generics, _, _) = generics.split_for_impl();

        quote! {
            #[automatically_derived]
            impl #impl_generics ::core::convert::From< #enum_life #name #ty_generics > for #discriminants_name #where_clause {
                #[inline]
                fn from(val: #enum_life #name #ty_generics) -> #discriminants_name {
                    #from_fn_body
                }
            }
        }
    };

    // For now, only implement IntoDiscriminant if the user has not overriden the visibility.
    let impl_into_discriminant = match type_properties.discriminant_vis {
        // If the visibilty is unspecified or `pub` then we implement IntoDiscriminant
        None | Some(syn::Visibility::Public(..)) => quote! {
            #[automatically_derived]
            impl #impl_generics #strum_module_path::IntoDiscriminant for #name #ty_generics #where_clause {
                type Discriminant = #discriminants_name;

                #[inline]
                fn discriminant(&self) -> Self::Discriminant {
                    <Self::Discriminant as ::core::convert::From<&Self>>::from(self)
                }
            }
        },
        // If it's something restricted such as `pub(super)` then we skip implementing the
        // trait for now. There are certainly scenarios where they could be equivalent, but
        // as a heuristic, if someone is overriding the visibility, it's because they want
        // the discriminant type to be less visible than the original type.
        _ => quote! {},
    };

    Ok(quote! {
        #docs
        #derives
        #repr
        #(#[ #pass_though_attributes ])*
        #discriminants_vis enum #discriminants_name {
            #(#discriminants),*
        }

        #impl_into_discriminant
        #impl_from
        #impl_from_ref
    })
}
