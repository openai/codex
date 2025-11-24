//! Implementation of a [`From`] derive macro.

use std::{
    any::{Any, TypeId},
    iter,
};

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens as _, TokenStreamExt as _};
use syn::{
    parse::{Parse, ParseStream},
    parse_quote,
    spanned::Spanned as _,
    token,
};

use crate::utils::{
    attr::{self, ParseMultiple as _},
    polyfill, Either, Spanning,
};

/// Expands a [`From`] derive macro.
pub fn expand(input: &syn::DeriveInput, _: &'static str) -> syn::Result<TokenStream> {
    let attr_name = format_ident!("from");

    match &input.data {
        syn::Data::Struct(data) => Expansion {
            attrs: StructAttribute::parse_attrs_with(
                &input.attrs,
                &attr_name,
                &ConsiderLegacySyntax {
                    fields: &data.fields,
                },
            )?
            .map(|attr| attr.into_inner().into())
            .as_ref(),
            ident: &input.ident,
            variant: None,
            fields: &data.fields,
            generics: &input.generics,
            has_explicit_from: false,
        }
        .expand(),
        syn::Data::Enum(data) => {
            let mut has_explicit_from = false;
            let attrs = data
                .variants
                .iter()
                .map(|variant| {
                    let attr = VariantAttribute::parse_attrs_with(
                        &variant.attrs,
                        &attr_name,
                        &ConsiderLegacySyntax {
                            fields: &variant.fields,
                        },
                    )?
                    .map(Spanning::into_inner);
                    if matches!(
                        attr,
                        Some(
                            VariantAttribute::Empty(_)
                                | VariantAttribute::Types(_)
                                | VariantAttribute::Forward(_)
                        ),
                    ) {
                        has_explicit_from = true;
                    }
                    Ok(attr)
                })
                .collect::<syn::Result<Vec<_>>>()?;

            data.variants
                .iter()
                .zip(&attrs)
                .map(|(variant, attrs)| {
                    Expansion {
                        attrs: attrs.as_ref(),
                        ident: &input.ident,
                        variant: Some(&variant.ident),
                        fields: &variant.fields,
                        generics: &input.generics,
                        has_explicit_from,
                    }
                    .expand()
                })
                .collect()
        }
        syn::Data::Union(data) => Err(syn::Error::new(
            data.union_token.span(),
            "`From` cannot be derived for unions",
        )),
    }
}

/// Representation of a [`From`] derive macro struct container attribute.
///
/// ```rust,ignore
/// #[from(forward)]
/// #[from(<types>)]
/// ```
type StructAttribute = attr::Conversion;

/// Representation of a [`From`] derive macro enum variant attribute.
///
/// ```rust,ignore
/// #[from]
/// #[from(skip)] #[from(ignore)]
/// #[from(forward)]
/// #[from(<types>)]
/// ```
type VariantAttribute = attr::FieldConversion;

/// Expansion of a macro for generating [`From`] implementation of a struct or
/// enum.
struct Expansion<'a> {
    /// [`From`] attributes.
    ///
    /// As a [`VariantAttribute`] is superset of a [`StructAttribute`], we use
    /// it for both derives.
    attrs: Option<&'a VariantAttribute>,

    /// Struct or enum [`syn::Ident`].
    ///
    /// [`syn::Ident`]: struct@syn::Ident
    ident: &'a syn::Ident,

    /// Variant [`syn::Ident`] in case of enum expansion.
    ///
    /// [`syn::Ident`]: struct@syn::Ident
    variant: Option<&'a syn::Ident>,

    /// Struct or variant [`syn::Fields`].
    fields: &'a syn::Fields,

    /// Struct or enum [`syn::Generics`].
    generics: &'a syn::Generics,

    /// Indicator whether one of the enum variants has
    /// [`VariantAttribute::Empty`], [`VariantAttribute::Types`] or
    /// [`VariantAttribute::Forward`].
    ///
    /// Always [`false`] for structs.
    has_explicit_from: bool,
}

impl<'a> Expansion<'a> {
    /// Expands [`From`] implementations for a struct or an enum variant.
    fn expand(&self) -> syn::Result<TokenStream> {
        use crate::utils::FieldsExt as _;

        let ident = self.ident;
        let field_tys = self.fields.iter().map(|f| &f.ty).collect::<Vec<_>>();
        let (impl_gens, ty_gens, where_clause) = self.generics.split_for_impl();

        let skip_variant = self.has_explicit_from
            || (self.variant.is_some() && self.fields.is_empty());
        match (self.attrs, skip_variant) {
            (Some(VariantAttribute::Types(tys)), _) => {
                tys.0.iter().map(|ty| {
                    let variant = self.variant.iter();

                    let mut from_tys = self.fields.validate_type(ty)?;
                    let init = self.expand_fields(|ident, ty, index| {
                        let ident = ident.into_iter();
                        let index = index.into_iter();
                        let from_ty = from_tys.next().unwrap_or_else(|| unreachable!());
                        quote! {
                            #( #ident: )* <#ty as derive_more::From<#from_ty>>::from(
                                value #( .#index )*
                            ),
                        }
                    });

                    Ok(quote! {
                        #[automatically_derived]
                        impl #impl_gens derive_more::From<#ty> for #ident #ty_gens #where_clause {
                            #[inline]
                            fn from(value: #ty) -> Self {
                                #ident #( :: #variant )* #init
                            }
                        }
                    })
                })
                .collect()
            }
            (Some(VariantAttribute::Empty(_)), _) | (None, false) => {
                let variant = self.variant.iter();
                let init = self.expand_fields(|ident, _, index| {
                    let ident = ident.into_iter();
                    let index = index.into_iter();
                    quote! { #( #ident: )* value #( . #index )*, }
                });

                Ok(quote! {
                    #[automatically_derived]
                    impl #impl_gens derive_more::From<(#( #field_tys ),*)> for #ident #ty_gens #where_clause {
                        #[inline]
                        fn from(value: (#( #field_tys ),*)) -> Self {
                            #ident #( :: #variant )* #init
                        }
                    }
                })
            }
            (Some(VariantAttribute::Forward(_)), _) => {
                let mut i = 0;
                let mut gen_idents = Vec::with_capacity(self.fields.len());
                let init = self.expand_fields(|ident, ty, index| {
                    let ident = ident.into_iter();
                    let index = index.into_iter();
                    let gen_ident = format_ident!("__FromT{i}");
                    let out = quote! {
                        #( #ident: )* <#ty as derive_more::From<#gen_ident>>::from(
                            value #( .#index )*
                        ),
                    };
                    gen_idents.push(gen_ident);
                    i += 1;
                    out
                });

                let variant = self.variant.iter();
                let generics = {
                    let mut generics = self.generics.clone();
                    for (ty, ident) in field_tys.iter().zip(&gen_idents) {
                        generics.make_where_clause().predicates.push(
                            parse_quote! { #ty: derive_more::From<#ident> },
                        );
                        generics
                            .params
                            .push(syn::TypeParam::from(ident.clone()).into());
                    }
                    generics
                };
                let (impl_gens, _, where_clause) = generics.split_for_impl();

                Ok(quote! {
                    #[automatically_derived]
                    impl #impl_gens derive_more::From<(#( #gen_idents ),*)> for #ident #ty_gens #where_clause {
                        #[inline]
                        fn from(value: (#( #gen_idents ),*)) -> Self {
                            #ident #(:: #variant)* #init
                        }
                    }
                })
            }
            (Some(VariantAttribute::Skip(_)), _) | (None, true) => {
                Ok(TokenStream::new())
            }
        }
    }

    /// Expands fields initialization wrapped into [`token::Brace`]s in case of
    /// [`syn::FieldsNamed`], or [`token::Paren`] in case of
    /// [`syn::FieldsUnnamed`].
    ///
    /// [`token::Brace`]: struct@token::Brace
    /// [`token::Paren`]: struct@token::Paren
    fn expand_fields(
        &self,
        mut wrap: impl FnMut(
            Option<&syn::Ident>,
            &syn::Type,
            Option<syn::Index>,
        ) -> TokenStream,
    ) -> TokenStream {
        let surround = match self.fields {
            syn::Fields::Named(_) | syn::Fields::Unnamed(_) => {
                Some(|tokens| match self.fields {
                    syn::Fields::Named(named) => {
                        let mut out = TokenStream::new();
                        named
                            .brace_token
                            .surround(&mut out, |out| out.append_all(tokens));
                        out
                    }
                    syn::Fields::Unnamed(unnamed) => {
                        let mut out = TokenStream::new();
                        unnamed
                            .paren_token
                            .surround(&mut out, |out| out.append_all(tokens));
                        out
                    }
                    syn::Fields::Unit => unreachable!(),
                })
            }
            syn::Fields::Unit => None,
        };

        surround
            .map(|surround| {
                surround(if self.fields.len() == 1 {
                    let field = self
                        .fields
                        .iter()
                        .next()
                        .unwrap_or_else(|| unreachable!("self.fields.len() == 1"));
                    wrap(field.ident.as_ref(), &field.ty, None)
                } else {
                    self.fields
                        .iter()
                        .enumerate()
                        .map(|(i, field)| {
                            wrap(field.ident.as_ref(), &field.ty, Some(i.into()))
                        })
                        .collect()
                })
            })
            .unwrap_or_default()
    }
}

/// [`attr::Parser`] considering legacy syntax for [`attr::Types`] and emitting [`legacy_error`], if
/// any occurs.
struct ConsiderLegacySyntax<'a> {
    /// [`syn::Fields`] of a struct or enum variant, the attribute is parsed for.
    fields: &'a syn::Fields,
}

impl attr::Parser for ConsiderLegacySyntax<'_> {
    fn parse<T: Parse + Any>(&self, input: ParseStream<'_>) -> syn::Result<T> {
        if TypeId::of::<T>() == TypeId::of::<attr::Types>() {
            let ahead = input.fork();
            if let Ok(p) = ahead.parse::<syn::Path>() {
                if p.is_ident("types") {
                    return legacy_error(&ahead, input.span(), self.fields);
                }
            }
        }
        T::parse(input)
    }
}

/// Constructs a [`syn::Error`] for legacy syntax: `#[from(types(i32, "&str"))]`.
fn legacy_error<T>(
    tokens: ParseStream<'_>,
    span: Span,
    fields: &syn::Fields,
) -> syn::Result<T> {
    let content;
    syn::parenthesized!(content in tokens);

    let types = content
        .parse_terminated(polyfill::NestedMeta::parse, token::Comma)?
        .into_iter()
        .map(|meta| {
            let value = match meta {
                polyfill::NestedMeta::Meta(meta) => {
                    meta.into_token_stream().to_string()
                }
                polyfill::NestedMeta::Lit(syn::Lit::Str(str)) => str.value(),
                polyfill::NestedMeta::Lit(_) => unreachable!(),
            };
            if fields.len() > 1 {
                format!(
                    "({})",
                    fields
                        .iter()
                        .map(|_| value.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
            } else {
                value
            }
        })
        .chain(match fields.len() {
            0 => Either::Left(iter::empty()),
            1 => Either::Right(iter::once(
                fields
                    .iter()
                    .next()
                    .unwrap_or_else(|| unreachable!("fields.len() == 1"))
                    .ty
                    .to_token_stream()
                    .to_string(),
            )),
            _ => Either::Right(iter::once(format!(
                "({})",
                fields
                    .iter()
                    .map(|f| f.ty.to_token_stream().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        })
        .collect::<Vec<_>>()
        .join(", ");

    Err(syn::Error::new(
        span,
        format!("legacy syntax, remove `types` and use `{types}` instead"),
    ))
}
