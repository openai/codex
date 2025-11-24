//! Implementation of a [`TryFrom`] derive macro.

use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::spanned::Spanned as _;

use crate::utils::{
    attr::{self, ParseMultiple as _},
    Spanning,
};

/// Expands a [`TryFrom`] derive macro.
pub fn expand(input: &syn::DeriveInput, _: &'static str) -> syn::Result<TokenStream> {
    match &input.data {
        syn::Data::Struct(data) => Err(syn::Error::new(
            data.struct_token.span(),
            "`TryFrom` cannot be derived for structs",
        )),
        syn::Data::Enum(data) => Ok(Expansion {
            repr: attr::ReprInt::parse_attrs(&input.attrs, &format_ident!("repr"))?
                .map(Spanning::into_inner)
                .unwrap_or_default(),
            attr: ItemAttribute::parse_attrs(&input.attrs, &format_ident!("try_from"))?
                .map(|attr| {
                    if matches!(attr.item, ItemAttribute::Types(_)) {
                        Err(syn::Error::new(
                            attr.span,
                            "`#[try_from(repr(...))]` attribute is not supported yet",
                        ))
                    } else {
                        Ok(attr.item)
                    }
                })
                .transpose()?,
            ident: input.ident.clone(),
            generics: input.generics.clone(),
            variants: data.variants.clone().into_iter().collect(),
        }
        .into_token_stream()),
        syn::Data::Union(data) => Err(syn::Error::new(
            data.union_token.span(),
            "`TryFrom` cannot be derived for unions",
        )),
    }
}

/// Representation of a [`TryFrom`] derive macro struct item attribute.
///
/// ```rust,ignore
/// #[try_from(repr)]
/// #[try_from(repr(<types>))]
/// ```
type ItemAttribute = attr::ReprConversion;

/// Expansion of a macro for generating [`TryFrom`] implementation of an enum.
struct Expansion {
    /// `#[repr(u/i*)]` of the enum.
    repr: attr::ReprInt,

    /// [`ItemAttribute`] of the enum.
    attr: Option<ItemAttribute>,

    /// [`syn::Ident`] of the enum.
    ///
    /// [`syn::Ident`]: struct@syn::Ident
    ident: syn::Ident,

    /// [`syn::Generics`] of the enum.
    generics: syn::Generics,

    /// [`syn::Variant`]s of the enum.
    variants: Vec<syn::Variant>,
}

impl ToTokens for Expansion {
    /// Expands [`TryFrom`] implementations for a struct.
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if self.attr.is_none() {
            return;
        }

        let ident = &self.ident;
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();

        let repr_ty = &self.repr.ty();

        let mut last_discriminant = quote! { 0 };
        let mut inc = 0usize;
        let (consts, (discriminants, variants)): (
            Vec<syn::Ident>,
            (Vec<TokenStream>, Vec<TokenStream>),
        ) = self
            .variants
            .iter()
            .filter_map(
                |syn::Variant {
                     ident,
                     fields,
                     discriminant,
                     ..
                 }| {
                    if let Some(d) = discriminant {
                        last_discriminant = d.1.to_token_stream();
                        inc = 0;
                    }
                    let ret = {
                        let inc = Literal::usize_unsuffixed(inc);
                        fields.is_empty().then_some((
                            format_ident!("__DISCRIMINANT_{ident}"),
                            (
                                quote! { #last_discriminant + #inc },
                                quote! { #ident #fields },
                            ),
                        ))
                    };
                    inc += 1;
                    ret
                },
            )
            .unzip();

        quote! {
            #[automatically_derived]
            impl #impl_generics derive_more::TryFrom<#repr_ty #ty_generics> for #ident #where_clause {
                type Error = derive_more::TryFromReprError<#repr_ty>;

                #[allow(non_upper_case_globals)]
                #[inline]
                fn try_from(val: #repr_ty) -> derive_more::core::result::Result<Self, Self::Error> {
                    #( const #consts: #repr_ty = #discriminants; )*
                    match val {
                        #(#consts => derive_more::core::result::Result::Ok(#ident::#variants),)*
                        _ => derive_more::core::result::Result::Err(
                            derive_more::TryFromReprError::new(val)
                        ),
                    }
                }
            }
        }.to_tokens(tokens);
    }
}
