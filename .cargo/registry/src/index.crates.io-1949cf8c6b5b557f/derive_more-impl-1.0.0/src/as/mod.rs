//! Implementations of [`AsRef`]/[`AsMut`] derive macros.

pub(crate) mod r#mut;
pub(crate) mod r#ref;

use std::{borrow::Cow, iter};

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_quote, spanned::Spanned, Token};

use crate::utils::{
    attr::{self, ParseMultiple as _},
    Either, GenericsSearch, Spanning,
};

/// Expands an [`AsRef`]/[`AsMut`] derive macro.
pub fn expand(
    input: &syn::DeriveInput,
    trait_info: ExpansionCtx<'_>,
) -> syn::Result<TokenStream> {
    let (trait_ident, attr_name, _) = trait_info;

    let data = match &input.data {
        syn::Data::Struct(data) => Ok(data),
        syn::Data::Enum(e) => Err(syn::Error::new(
            e.enum_token.span(),
            format!("`{trait_ident}` cannot be derived for enums"),
        )),
        syn::Data::Union(u) => Err(syn::Error::new(
            u.union_token.span(),
            format!("`{trait_ident}` cannot be derived for unions"),
        )),
    }?;

    let expansions = if let Some(attr) =
        StructAttribute::parse_attrs(&input.attrs, attr_name)?
    {
        if data.fields.len() != 1 {
            return Err(syn::Error::new(
                if data.fields.is_empty() {
                    data.struct_token.span
                } else {
                    data.fields.span()
                },
                format!(
                    "`#[{attr_name}(...)]` attribute can only be placed on structs with exactly \
                     one field",
                ),
            ));
        }

        let field = data.fields.iter().next().unwrap();
        if FieldAttribute::parse_attrs(&field.attrs, attr_name)?.is_some() {
            return Err(syn::Error::new(
                field.span(),
                format!("`#[{attr_name}(...)]` cannot be placed on both struct and its field"),
            ));
        }

        vec![Expansion {
            trait_info,
            ident: &input.ident,
            generics: &input.generics,
            field,
            field_index: 0,
            conversions: Some(attr.into_inner()),
        }]
    } else {
        let attrs = data
            .fields
            .iter()
            .map(|field| FieldAttribute::parse_attrs(&field.attrs, attr_name))
            .collect::<syn::Result<Vec<_>>>()?;

        let present_attrs = attrs.iter().filter_map(Option::as_ref).collect::<Vec<_>>();

        let all = present_attrs
            .iter()
            .all(|attr| matches!(attr.item, FieldAttribute::Skip(_)));

        if !all {
            if let Some(skip_attr) = present_attrs.iter().find_map(|attr| {
                if let FieldAttribute::Skip(skip) = &attr.item {
                    Some(attr.as_ref().map(|_| skip))
                } else {
                    None
                }
            }) {
                return Err(syn::Error::new(
                    skip_attr.span(),
                    format!(
                        "`#[{attr_name}({})]` cannot be used in the same struct with other \
                         `#[{attr_name}(...)]` attributes",
                        skip_attr.name(),
                    ),
                ));
            }
        }

        if all {
            data.fields
                .iter()
                .enumerate()
                .zip(attrs)
                .filter_map(|((i, field), attr)| {
                    attr.is_none().then_some(Expansion {
                        trait_info,
                        ident: &input.ident,
                        generics: &input.generics,
                        field,
                        field_index: i,
                        conversions: None,
                    })
                })
                .collect()
        } else {
            data.fields
                .iter()
                .enumerate()
                .zip(attrs)
                .filter_map(|((i, field), attr)| match attr.map(Spanning::into_inner) {
                    Some(
                        attr @ (FieldAttribute::Empty(_)
                        | FieldAttribute::Forward(_)
                        | FieldAttribute::Types(_)),
                    ) => Some(Expansion {
                        trait_info,
                        ident: &input.ident,
                        generics: &input.generics,
                        field,
                        field_index: i,
                        conversions: attr.into(),
                    }),
                    Some(FieldAttribute::Skip(_)) => unreachable!(),
                    None => None,
                })
                .collect()
        }
    };
    Ok(expansions
        .into_iter()
        .map(ToTokens::into_token_stream)
        .collect())
}

/// Type alias for an expansion context:
/// - [`syn::Ident`] of the derived trait.
/// - [`syn::Ident`] of the derived trait method.
/// - Optional `mut` token indicating [`AsMut`] expansion.
///
/// [`syn::Ident`]: struct@syn::Ident
type ExpansionCtx<'a> = (&'a syn::Ident, &'a syn::Ident, Option<&'a Token![mut]>);

/// Expansion of a macro for generating [`AsRef`]/[`AsMut`] implementations for a single field of a
/// struct.
struct Expansion<'a> {
    /// [`ExpansionCtx`] of the derived trait.
    trait_info: ExpansionCtx<'a>,

    /// [`syn::Ident`] of the struct.
    ///
    /// [`syn::Ident`]: struct@syn::Ident
    ident: &'a syn::Ident,

    /// [`syn::Generics`] of the struct.
    generics: &'a syn::Generics,

    /// [`syn::Field`] of the struct.
    field: &'a syn::Field,

    /// Index of the [`syn::Field`].
    field_index: usize,

    /// Attribute specifying which conversions should be generated.
    conversions: Option<attr::Conversion>,
}

impl<'a> ToTokens for Expansion<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let field_ty = &self.field.ty;
        let field_ident = self.field.ident.as_ref().map_or_else(
            || Either::Right(syn::Index::from(self.field_index)),
            Either::Left,
        );

        let (trait_ident, method_ident, mut_) = &self.trait_info;
        let ty_ident = &self.ident;

        let field_ref = quote! { & #mut_ self.#field_ident };

        let generics_search = GenericsSearch {
            types: self.generics.type_params().map(|p| &p.ident).collect(),
            lifetimes: self
                .generics
                .lifetimes()
                .map(|p| &p.lifetime.ident)
                .collect(),
            consts: self.generics.const_params().map(|p| &p.ident).collect(),
        };
        let field_contains_generics = generics_search.any_in(field_ty);

        let is_blanket =
            matches!(&self.conversions, Some(attr::Conversion::Forward(_)));

        let return_tys = match &self.conversions {
            Some(attr::Conversion::Forward(_)) => {
                Either::Left(iter::once(Cow::Owned(parse_quote! { __AsT })))
            }
            Some(attr::Conversion::Types(tys)) => {
                Either::Right(tys.0.iter().map(Cow::Borrowed))
            }
            None => Either::Left(iter::once(Cow::Borrowed(field_ty))),
        };

        for return_ty in return_tys {
            /// Kind of a generated implementation, chosen based on attribute arguments.
            enum ImplKind {
                /// Returns a reference to a field.
                Direct,

                /// Forwards `as_ref`/`as_mut` call on a field.
                Forwarded,

                /// Uses autoref-based specialization to determine whether to use direct or
                /// forwarded implementation, based on whether the field and the return type match.
                ///
                /// Doesn't work when generics are involved.
                Specialized,
            }

            let impl_kind = if is_blanket {
                ImplKind::Forwarded
            } else if field_ty == return_ty.as_ref() {
                ImplKind::Direct
            } else if field_contains_generics || generics_search.any_in(&return_ty) {
                ImplKind::Forwarded
            } else {
                ImplKind::Specialized
            };

            let trait_ty = quote! {
                derive_more::#trait_ident <#return_ty>
            };

            let generics = match &impl_kind {
                ImplKind::Forwarded => {
                    let mut generics = self.generics.clone();
                    generics
                        .make_where_clause()
                        .predicates
                        .push(parse_quote! { #field_ty: #trait_ty });
                    if is_blanket {
                        generics
                            .params
                            .push(parse_quote! { #return_ty: ?derive_more::core::marker::Sized });
                    }
                    Cow::Owned(generics)
                }
                ImplKind::Direct | ImplKind::Specialized => {
                    Cow::Borrowed(self.generics)
                }
            };
            let (impl_gens, _, where_clause) = generics.split_for_impl();
            let (_, ty_gens, _) = self.generics.split_for_impl();

            let body = match &impl_kind {
                ImplKind::Direct => Cow::Borrowed(&field_ref),
                ImplKind::Forwarded => Cow::Owned(quote! {
                    <#field_ty as #trait_ty>::#method_ident(#field_ref)
                }),
                ImplKind::Specialized => Cow::Owned(quote! {
                    use derive_more::__private::ExtractRef as _;

                    let conv =
                        <derive_more::__private::Conv<& #mut_ #field_ty, #return_ty>
                         as derive_more::core::default::Default>::default();
                    (&&conv).__extract_ref(#field_ref)
                }),
            };

            quote! {
                #[automatically_derived]
                impl #impl_gens #trait_ty for #ty_ident #ty_gens #where_clause {
                    #[inline]
                    fn #method_ident(& #mut_ self) -> & #mut_ #return_ty {
                        #body
                    }
                }
            }
            .to_tokens(tokens);
        }
    }
}

/// Representation of an [`AsRef`]/[`AsMut`] derive macro struct container attribute.
///
/// ```rust,ignore
/// #[as_ref(forward)]
/// #[as_ref(<types>)]
/// ```
type StructAttribute = attr::Conversion;

/// Representation of an [`AsRef`]/[`AsMut`] derive macro field attribute.
///
/// ```rust,ignore
/// #[as_ref]
/// #[as_ref(skip)] #[as_ref(ignore)]
/// #[as_ref(forward)]
/// #[as_ref(<types>)]
/// ```
type FieldAttribute = attr::FieldConversion;
