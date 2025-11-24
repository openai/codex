//! Implementation of a [`fmt::Debug`] derive macro.
//!
//! [`fmt::Debug`]: std::fmt::Debug

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ext::IdentExt as _, parse_quote, spanned::Spanned as _};

use crate::utils::{
    attr::{self, ParseMultiple as _},
    Either, Spanning,
};

use super::{
    trait_name_to_attribute_name, ContainerAttributes, ContainsGenericsExt as _,
    FmtAttribute,
};

/// Expands a [`fmt::Debug`] derive macro.
///
/// [`fmt::Debug`]: std::fmt::Debug
pub fn expand(input: &syn::DeriveInput, _: &str) -> syn::Result<TokenStream> {
    let attr_name = format_ident!("{}", trait_name_to_attribute_name("Debug"));

    let attrs = ContainerAttributes::parse_attrs(&input.attrs, &attr_name)?
        .map(Spanning::into_inner)
        .unwrap_or_default();
    let ident = &input.ident;

    let type_params = input
        .generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Type(t) => Some(&t.ident),
            syn::GenericParam::Const(..) | syn::GenericParam::Lifetime(..) => None,
        })
        .collect::<Vec<_>>();

    let (bounds, body) = match &input.data {
        syn::Data::Struct(s) => {
            expand_struct(attrs, ident, s, &type_params, &attr_name)
        }
        syn::Data::Enum(e) => expand_enum(attrs, e, &type_params, &attr_name),
        syn::Data::Union(_) => {
            return Err(syn::Error::new(
                input.span(),
                "`Debug` cannot be derived for unions",
            ));
        }
    }?;

    let (impl_gens, ty_gens, where_clause) = {
        let (impl_gens, ty_gens, where_clause) = input.generics.split_for_impl();
        let mut where_clause = where_clause
            .cloned()
            .unwrap_or_else(|| parse_quote! { where });
        where_clause.predicates.extend(bounds);
        (impl_gens, ty_gens, where_clause)
    };

    Ok(quote! {
        #[allow(unreachable_code)] // omit warnings for `!` and other unreachable types
        #[automatically_derived]
        impl #impl_gens derive_more::core::fmt::Debug for #ident #ty_gens #where_clause {
            #[inline]
            fn fmt(
                &self, __derive_more_f: &mut derive_more::core::fmt::Formatter<'_>
            ) -> derive_more::core::fmt::Result {
                #body
            }
        }
    })
}

/// Expands a [`fmt::Debug`] derive macro for the provided struct.
///
/// [`fmt::Debug`]: std::fmt::Debug
fn expand_struct(
    attrs: ContainerAttributes,
    ident: &syn::Ident,
    s: &syn::DataStruct,
    type_params: &[&syn::Ident],
    attr_name: &syn::Ident,
) -> syn::Result<(Vec<syn::WherePredicate>, TokenStream)> {
    let s = Expansion {
        attr: &attrs,
        fields: &s.fields,
        type_params,
        ident,
        attr_name,
    };
    s.validate_attrs()?;
    let bounds = s.generate_bounds()?;
    let body = s.generate_body()?;

    let vars = s.fields.iter().enumerate().map(|(i, f)| {
        let var = f.ident.clone().unwrap_or_else(|| format_ident!("_{i}"));
        let member = f
            .ident
            .clone()
            .map_or_else(|| syn::Member::Unnamed(i.into()), syn::Member::Named);
        quote! { let #var = &self.#member; }
    });

    let body = quote! {
        #( #vars )*
        #body
    };

    Ok((bounds, body))
}

/// Expands a [`fmt::Debug`] derive macro for the provided enum.
///
/// [`fmt::Debug`]: std::fmt::Debug
fn expand_enum(
    mut attrs: ContainerAttributes,
    e: &syn::DataEnum,
    type_params: &[&syn::Ident],
    attr_name: &syn::Ident,
) -> syn::Result<(Vec<syn::WherePredicate>, TokenStream)> {
    if let Some(enum_fmt) = attrs.fmt.as_ref() {
        return Err(syn::Error::new_spanned(
            enum_fmt,
            format!(
                "`#[{attr_name}(\"...\", ...)]` attribute is not allowed on enum, place it on its \
                 variants instead",
            ),
        ));
    }

    let (bounds, match_arms) = e.variants.iter().try_fold(
        (Vec::new(), TokenStream::new()),
        |(mut bounds, mut arms), variant| {
            let ident = &variant.ident;

            attrs.fmt = variant
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("debug"))
                .try_fold(None, |mut attrs, attr| {
                    let attr = attr.parse_args::<FmtAttribute>()?;
                    attrs.replace(attr).map_or(Ok(()), |dup| {
                        Err(syn::Error::new(
                            dup.span(),
                            format!(
                                "multiple `#[{attr_name}(\"...\", ...)]` attributes aren't allowed",
                            ),
                        ))
                    })?;
                    Ok::<_, syn::Error>(attrs)
                })?;

            let v = Expansion {
                attr: &attrs,
                fields: &variant.fields,
                type_params,
                ident,
                attr_name,
            };
            v.validate_attrs()?;
            let arm_body = v.generate_body()?;
            bounds.extend(v.generate_bounds()?);

            let fields_idents =
                variant.fields.iter().enumerate().map(|(i, f)| {
                    f.ident.clone().unwrap_or_else(|| format_ident!("_{i}"))
                });
            let matcher = match variant.fields {
                syn::Fields::Named(_) => {
                    quote! { Self::#ident { #( #fields_idents ),* } }
                }
                syn::Fields::Unnamed(_) => {
                    quote! { Self::#ident ( #( #fields_idents ),* ) }
                }
                syn::Fields::Unit => quote! { Self::#ident },
            };

            arms.extend([quote! { #matcher => { #arm_body }, }]);

            Ok::<_, syn::Error>((bounds, arms))
        },
    )?;

    let body = match_arms
        .is_empty()
        .then(|| quote! { match *self {} })
        .unwrap_or_else(|| quote! { match self { #match_arms } });

    Ok((bounds, body))
}

/// Representation of a [`fmt::Debug`] derive macro field attribute.
///
/// ```rust,ignore
/// #[debug(skip)]
/// #[debug("<fmt-literal>", <fmt-args>)]
/// ```
///
/// [`fmt::Debug`]: std::fmt::Debug
type FieldAttribute = Either<attr::Skip, FmtAttribute>;

/// Helper struct to generate [`Debug::fmt()`] implementation body and trait
/// bounds for a struct or an enum variant.
///
/// [`Debug::fmt()`]: std::fmt::Debug::fmt()
#[derive(Debug)]
struct Expansion<'a> {
    attr: &'a ContainerAttributes,

    /// Struct or enum [`Ident`](struct@syn::Ident).
    ident: &'a syn::Ident,

    /// Struct or enum [`syn::Fields`].
    fields: &'a syn::Fields,

    /// Type parameters in this struct or enum.
    type_params: &'a [&'a syn::Ident],

    /// Name of the attributes, considered by this macro.
    attr_name: &'a syn::Ident,
}

impl Expansion<'_> {
    /// Validates attributes of this [`Expansion`] to be consistent.
    fn validate_attrs(&self) -> syn::Result<()> {
        if self.attr.fmt.is_some() {
            for field_attr in self
                .fields
                .iter()
                .map(|f| FieldAttribute::parse_attrs(&f.attrs, self.attr_name))
            {
                if let Some(FieldAttribute::Right(fmt_attr)) =
                    field_attr?.map(Spanning::into_inner)
                {
                    return Err(syn::Error::new_spanned(
                        fmt_attr,
                        "`#[debug(...)]` attributes are not allowed on fields when \
                         `#[debug(\"...\", ...)]` is specified on struct or variant",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Generates [`Debug::fmt()`] implementation for a struct or an enum variant.
    ///
    /// [`Debug::fmt()`]: std::fmt::Debug::fmt()
    fn generate_body(&self) -> syn::Result<TokenStream> {
        if let Some(fmt) = &self.attr.fmt {
            return Ok(
                if let Some((expr, trait_ident)) =
                    fmt.transparent_call_on_fields(self.fields)
                {
                    quote! { derive_more::core::fmt::#trait_ident::fmt(#expr, __derive_more_f) }
                } else {
                    let deref_args = fmt.additional_deref_args(self.fields);

                    quote! { derive_more::core::write!(__derive_more_f, #fmt, #(#deref_args),*) }
                },
            );
        };

        match self.fields {
            syn::Fields::Unit => {
                let ident = self.ident.to_string();
                Ok(quote! {
                    derive_more::core::fmt::Formatter::write_str(
                        __derive_more_f,
                        #ident,
                    )
                })
            }
            syn::Fields::Unnamed(unnamed) => {
                let mut exhaustive = true;
                let ident_str = self.ident.to_string();

                let out = quote! {
                    &mut derive_more::__private::debug_tuple(
                        __derive_more_f,
                        #ident_str,
                    )
                };
                let out = unnamed.unnamed.iter().enumerate().try_fold(
                    out,
                    |out, (i, field)| match FieldAttribute::parse_attrs(
                        &field.attrs,
                        self.attr_name,
                    )?
                    .map(Spanning::into_inner)
                    {
                        Some(FieldAttribute::Left(_skip)) => {
                            exhaustive = false;
                            Ok::<_, syn::Error>(out)
                        }
                        Some(FieldAttribute::Right(fmt_attr)) => {
                            let deref_args = fmt_attr.additional_deref_args(self.fields);

                            Ok(quote! {
                                derive_more::__private::DebugTuple::field(
                                    #out,
                                    &derive_more::core::format_args!(#fmt_attr, #(#deref_args),*),
                                )
                            })
                        }
                        None => {
                            let ident = format_ident!("_{i}");
                            Ok(quote! {
                                derive_more::__private::DebugTuple::field(#out, &#ident)
                            })
                        }
                    },
                )?;
                Ok(if exhaustive {
                    quote! { derive_more::__private::DebugTuple::finish(#out) }
                } else {
                    quote! { derive_more::__private::DebugTuple::finish_non_exhaustive(#out) }
                })
            }
            syn::Fields::Named(named) => {
                let mut exhaustive = true;
                let ident = self.ident.to_string();

                let out = quote! {
                    &mut derive_more::core::fmt::Formatter::debug_struct(
                        __derive_more_f,
                        #ident,
                    )
                };
                let out = named.named.iter().try_fold(out, |out, field| {
                    let field_ident = field.ident.as_ref().unwrap_or_else(|| {
                        unreachable!("`syn::Fields::Named`");
                    });
                    let field_str = field_ident.unraw().to_string();
                    match FieldAttribute::parse_attrs(&field.attrs, self.attr_name)?
                        .map(Spanning::into_inner)
                    {
                        Some(FieldAttribute::Left(_skip)) => {
                            exhaustive = false;
                            Ok::<_, syn::Error>(out)
                        }
                        Some(FieldAttribute::Right(fmt_attr)) => {
                            let deref_args =
                                fmt_attr.additional_deref_args(self.fields);

                            Ok(quote! {
                                derive_more::core::fmt::DebugStruct::field(
                                    #out,
                                    #field_str,
                                    &derive_more::core::format_args!(
                                        #fmt_attr, #(#deref_args),*
                                    ),
                                )
                            })
                        }
                        None => Ok(quote! {
                            derive_more::core::fmt::DebugStruct::field(
                                #out, #field_str, &#field_ident
                            )
                        }),
                    }
                })?;
                Ok(if exhaustive {
                    quote! { derive_more::core::fmt::DebugStruct::finish(#out) }
                } else {
                    quote! { derive_more::core::fmt::DebugStruct::finish_non_exhaustive(#out) }
                })
            }
        }
    }

    /// Generates trait bounds for a struct or an enum variant.
    fn generate_bounds(&self) -> syn::Result<Vec<syn::WherePredicate>> {
        let mut out = self.attr.bounds.0.clone().into_iter().collect::<Vec<_>>();

        if let Some(fmt) = self.attr.fmt.as_ref() {
            out.extend(fmt.bounded_types(self.fields).filter_map(
                |(ty, trait_name)| {
                    if !ty.contains_generics(self.type_params) {
                        return None;
                    }

                    let trait_ident = format_ident!("{trait_name}");

                    Some(parse_quote! { #ty: derive_more::core::fmt::#trait_ident })
                },
            ));
            Ok(out)
        } else {
            self.fields.iter().try_fold(out, |mut out, field| {
                let ty = &field.ty;

                if !ty.contains_generics(self.type_params) {
                    return Ok(out);
                }

                match FieldAttribute::parse_attrs(&field.attrs, self.attr_name)?
                    .map(Spanning::into_inner)
                {
                    Some(FieldAttribute::Right(fmt_attr)) => {
                        out.extend(fmt_attr.bounded_types(self.fields).map(
                            |(ty, trait_name)| {
                                let trait_ident = format_ident!("{trait_name}");

                                parse_quote! { #ty: derive_more::core::fmt::#trait_ident }
                            },
                        ));
                    }
                    Some(FieldAttribute::Left(_skip)) => {}
                    None => out.extend([parse_quote! { #ty: derive_more::core::fmt::Debug }]),
                }
                Ok(out)
            })
        }
    }
}
