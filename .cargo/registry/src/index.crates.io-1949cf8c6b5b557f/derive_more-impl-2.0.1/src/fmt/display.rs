//! Implementation of [`fmt::Display`]-like derive macros.

#[cfg(doc)]
use std::fmt;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ext::IdentExt as _, parse_quote, spanned::Spanned as _};

use crate::utils::{attr::ParseMultiple as _, Spanning};

use super::{
    trait_name_to_attribute_name, ContainerAttributes, ContainsGenericsExt as _,
    FmtAttribute,
};

/// Expands a [`fmt::Display`]-like derive macro.
///
/// Available macros:
/// - [`Binary`](fmt::Binary)
/// - [`Display`](fmt::Display)
/// - [`LowerExp`](fmt::LowerExp)
/// - [`LowerHex`](fmt::LowerHex)
/// - [`Octal`](fmt::Octal)
/// - [`Pointer`](fmt::Pointer)
/// - [`UpperExp`](fmt::UpperExp)
/// - [`UpperHex`](fmt::UpperHex)
pub fn expand(input: &syn::DeriveInput, trait_name: &str) -> syn::Result<TokenStream> {
    let trait_name = normalize_trait_name(trait_name);
    let attr_name = format_ident!("{}", trait_name_to_attribute_name(trait_name));

    let attrs = ContainerAttributes::parse_attrs(&input.attrs, &attr_name)?
        .map(Spanning::into_inner)
        .unwrap_or_default();
    let trait_ident = format_ident!("{trait_name}");
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

    let ctx: ExpansionCtx = (&attrs, &type_params, ident, &trait_ident, &attr_name);
    let (bounds, body) = match &input.data {
        syn::Data::Struct(s) => expand_struct(s, ctx),
        syn::Data::Enum(e) => expand_enum(e, ctx),
        syn::Data::Union(u) => expand_union(u, ctx),
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
        impl #impl_gens derive_more::core::fmt::#trait_ident for #ident #ty_gens #where_clause {
            fn fmt(
                &self, __derive_more_f: &mut derive_more::core::fmt::Formatter<'_>
            ) -> derive_more::core::fmt::Result {
                #body
            }
        }
    })
}

/// Type alias for an expansion context:
/// - [`ContainerAttributes`].
/// - Type parameters. Slice of [`syn::Ident`].
/// - Struct/enum/union [`syn::Ident`].
/// - Derived trait [`syn::Ident`].
/// - Attribute name [`syn::Ident`].
///
/// [`syn::Ident`]: struct@syn::Ident
type ExpansionCtx<'a> = (
    &'a ContainerAttributes,
    &'a [&'a syn::Ident],
    &'a syn::Ident,
    &'a syn::Ident,
    &'a syn::Ident,
);

/// Expands a [`fmt::Display`]-like derive macro for the provided struct.
fn expand_struct(
    s: &syn::DataStruct,
    (attrs, type_params, ident, trait_ident, _): ExpansionCtx<'_>,
) -> syn::Result<(Vec<syn::WherePredicate>, TokenStream)> {
    let s = Expansion {
        shared_attr: None,
        attrs,
        fields: &s.fields,
        type_params,
        trait_ident,
        ident,
    };
    let bounds = s.generate_bounds();
    let body = s.generate_body()?;

    let vars = s.fields.iter().enumerate().map(|(i, f)| {
        let var = f.ident.clone().unwrap_or_else(|| format_ident!("_{i}"));
        let member = f
            .ident
            .clone()
            .map_or_else(|| syn::Member::Unnamed(i.into()), syn::Member::Named);
        quote! {
            let #var = &self.#member;
        }
    });

    let body = quote! {
        #( #vars )*
        #body
    };

    Ok((bounds, body))
}

/// Expands a [`fmt`]-like derive macro for the provided enum.
fn expand_enum(
    e: &syn::DataEnum,
    (container_attrs, type_params, _, trait_ident, attr_name): ExpansionCtx<'_>,
) -> syn::Result<(Vec<syn::WherePredicate>, TokenStream)> {
    if let Some(shared_fmt) = &container_attrs.fmt {
        if shared_fmt
            .placeholders_by_arg("_variant")
            .any(|p| p.has_modifiers || p.trait_name != "Display")
        {
            // TODO: This limitation can be lifted, by analyzing the `shared_fmt` deeper and using
            //       `&dyn fmt::TraitName` for transparency instead of just `format_args!()` in the
            //       expansion.
            return Err(syn::Error::new(
                shared_fmt.span(),
                "shared format `_variant` placeholder cannot contain format specifiers",
            ));
        }
    }

    let (bounds, match_arms) = e.variants.iter().try_fold(
        (Vec::new(), TokenStream::new()),
        |(mut bounds, mut arms), variant| {
            let attrs = ContainerAttributes::parse_attrs(&variant.attrs, attr_name)?
                .map(Spanning::into_inner)
                .unwrap_or_default();
            let ident = &variant.ident;

            if attrs.fmt.is_none()
                && variant.fields.is_empty()
                && attr_name != "display"
            {
                return Err(syn::Error::new(
                    e.variants.span(),
                    format!(
                        "implicit formatting of unit enum variant is supported only for `Display` \
                         macro, use `#[{attr_name}(\"...\")]` to explicitly specify the formatting",
                    ),
                ));
            }

            let v = Expansion {
                shared_attr: container_attrs.fmt.as_ref(),
                attrs: &attrs,
                fields: &variant.fields,
                type_params,
                trait_ident,
                ident,
            };
            let arm_body = v.generate_body()?;
            bounds.extend(v.generate_bounds());

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

/// Expands a [`fmt::Display`]-like derive macro for the provided union.
fn expand_union(
    u: &syn::DataUnion,
    (attrs, _, _, _, attr_name): ExpansionCtx<'_>,
) -> syn::Result<(Vec<syn::WherePredicate>, TokenStream)> {
    let fmt = &attrs.fmt.as_ref().ok_or_else(|| {
        syn::Error::new(
            u.fields.span(),
            format!("unions must have `#[{attr_name}(\"...\", ...)]` attribute"),
        )
    })?;

    Ok((
        attrs.bounds.0.clone().into_iter().collect(),
        quote! { derive_more::core::write!(__derive_more_f, #fmt) },
    ))
}

/// Helper struct to generate [`Display::fmt()`] implementation body and trait
/// bounds for a struct or an enum variant.
///
/// [`Display::fmt()`]: fmt::Display::fmt()
#[derive(Debug)]
struct Expansion<'a> {
    /// [`FmtAttribute`] shared between all variants of an enum.
    ///
    /// [`None`] for a struct.
    shared_attr: Option<&'a FmtAttribute>,

    /// Derive macro [`ContainerAttributes`].
    attrs: &'a ContainerAttributes,

    /// Struct or enum [`syn::Ident`].
    ///
    /// [`syn::Ident`]: struct@syn::Ident
    ident: &'a syn::Ident,

    /// Struct or enum [`syn::Fields`].
    fields: &'a syn::Fields,

    /// Type parameters in this struct or enum.
    type_params: &'a [&'a syn::Ident],

    /// [`fmt`] trait [`syn::Ident`].
    ///
    /// [`syn::Ident`]: struct@syn::Ident
    trait_ident: &'a syn::Ident,
}

impl Expansion<'_> {
    /// Checks and indicates whether a top-level shared [`FmtAttribute`] is present in this
    /// [`Expansion`], and whether it has wrapping logic (e.g. uses `_variant` placeholder).
    fn shared_attr_info(&self) -> (bool, bool) {
        let shared_attr_contains_variant = self
            .shared_attr
            .map_or(true, |attr| attr.contains_arg("_variant"));
        // If `shared_attr` is a transparent call to `_variant`, then we consider it being absent.
        let has_shared_attr = self.shared_attr.is_some_and(|attr| {
            attr.transparent_call().map_or(true, |(_, called_trait)| {
                &called_trait != self.trait_ident || !shared_attr_contains_variant
            })
        });
        (
            has_shared_attr,
            has_shared_attr && shared_attr_contains_variant,
        )
    }

    /// Generates [`Display::fmt()`] implementation for a struct or an enum variant.
    ///
    /// # Errors
    ///
    /// In case [`FmtAttribute`] is [`None`] and [`syn::Fields`] length is greater than 1.
    ///
    /// [`Display::fmt()`]: fmt::Display::fmt()
    fn generate_body(&self) -> syn::Result<TokenStream> {
        let mut body = TokenStream::new();

        let (has_shared_attr, shared_attr_is_wrapping) = self.shared_attr_info();

        let wrap_into_shared_attr = match &self.attrs.fmt {
            Some(fmt) => {
                body = if shared_attr_is_wrapping {
                    let deref_args = fmt.additional_deref_args(self.fields);

                    quote! { &derive_more::core::format_args!(#fmt, #(#deref_args),*) }
                } else if let Some((expr, trait_ident)) =
                    fmt.transparent_call_on_fields(self.fields)
                {
                    quote! { derive_more::core::fmt::#trait_ident::fmt(#expr, __derive_more_f) }
                } else {
                    let deref_args = fmt.additional_deref_args(self.fields);

                    quote! { derive_more::core::write!(__derive_more_f, #fmt, #(#deref_args),*) }
                };
                shared_attr_is_wrapping
            }
            None => {
                if shared_attr_is_wrapping || !has_shared_attr {
                    body = if self.fields.is_empty() {
                        let ident_str = self.ident.unraw().to_string();

                        if shared_attr_is_wrapping {
                            quote! { #ident_str }
                        } else {
                            quote! { __derive_more_f.write_str(#ident_str) }
                        }
                    } else if self.fields.len() == 1 {
                        let field = self
                            .fields
                            .iter()
                            .next()
                            .unwrap_or_else(|| unreachable!("count() == 1"));
                        let ident =
                            field.ident.clone().unwrap_or_else(|| format_ident!("_0"));
                        let trait_ident = self.trait_ident;

                        if shared_attr_is_wrapping {
                            let placeholder =
                                trait_name_to_default_placeholder_literal(trait_ident);

                            quote! { &derive_more::core::format_args!(#placeholder, #ident) }
                        } else {
                            quote! {
                                derive_more::core::fmt::#trait_ident::fmt(#ident, __derive_more_f)
                            }
                        }
                    } else {
                        return Err(syn::Error::new(
                            self.fields.span(),
                            format!(
                                "struct or enum variant with more than 1 field must have \
                                 `#[{}(\"...\", ...)]` attribute",
                                trait_name_to_attribute_name(self.trait_ident),
                            ),
                        ));
                    };
                }
                has_shared_attr
            }
        };
        if wrap_into_shared_attr {
            if let Some(shared_fmt) = &self.shared_attr {
                let deref_args = shared_fmt.additional_deref_args(self.fields);

                let shared_body = if let Some((expr, trait_ident)) =
                    shared_fmt.transparent_call_on_fields(self.fields)
                {
                    quote! { derive_more::core::fmt::#trait_ident::fmt(#expr, __derive_more_f) }
                } else {
                    quote! {
                        derive_more::core::write!(__derive_more_f, #shared_fmt, #(#deref_args),*)
                    }
                };

                body = if body.is_empty() {
                    shared_body
                } else {
                    quote! { match #body { _variant => #shared_body } }
                }
            }
        }

        Ok(body)
    }

    /// Generates trait bounds for a struct or an enum variant.
    fn generate_bounds(&self) -> Vec<syn::WherePredicate> {
        let mut bounds = vec![];

        let (has_shared_attr, shared_attr_is_wrapping) = self.shared_attr_info();

        let mix_shared_attr_bounds = match &self.attrs.fmt {
            Some(attr) => {
                bounds.extend(
                    attr.bounded_types(self.fields)
                        .filter_map(|(ty, trait_name)| {
                            if !ty.contains_generics(self.type_params) {
                                return None;
                            }
                            let trait_ident = format_ident!("{trait_name}");

                            Some(parse_quote! { #ty: derive_more::core::fmt::#trait_ident })
                        })
                        .chain(self.attrs.bounds.0.clone()),
                );
                shared_attr_is_wrapping
            }
            None => {
                if shared_attr_is_wrapping || !has_shared_attr {
                    bounds.extend(self.fields.iter().next().and_then(|f| {
                        let ty = &f.ty;
                        if !ty.contains_generics(self.type_params) {
                            return None;
                        }
                        let trait_ident = &self.trait_ident;
                        Some(parse_quote! { #ty: derive_more::core::fmt::#trait_ident })
                    }));
                }
                has_shared_attr
            }
        };
        if mix_shared_attr_bounds {
            bounds.extend(
                self.shared_attr
                    .as_ref()
                    .unwrap()
                    .bounded_types(self.fields)
                    .filter_map(|(ty, trait_name)| {
                        if !ty.contains_generics(self.type_params) {
                            return None;
                        }
                        let trait_ident = format_ident!("{trait_name}");

                        Some(parse_quote! { #ty: derive_more::core::fmt::#trait_ident })
                    }),
            );
        }

        bounds
    }
}

/// Matches the provided derive macro `name` to appropriate actual trait name.
fn normalize_trait_name(name: &str) -> &'static str {
    match name {
        "Binary" => "Binary",
        "Display" => "Display",
        "LowerExp" => "LowerExp",
        "LowerHex" => "LowerHex",
        "Octal" => "Octal",
        "Pointer" => "Pointer",
        "UpperExp" => "UpperExp",
        "UpperHex" => "UpperHex",
        _ => unimplemented!(),
    }
}

/// Matches the provided [`fmt`] trait `name` to its default formatting placeholder.
fn trait_name_to_default_placeholder_literal(name: &syn::Ident) -> &'static str {
    match () {
        _ if name == "Binary" => "{:b}",
        _ if name == "Debug" => "{:?}",
        _ if name == "Display" => "{}",
        _ if name == "LowerExp" => "{:e}",
        _ if name == "LowerHex" => "{:x}",
        _ if name == "Octal" => "{:o}",
        _ if name == "Pointer" => "{:p}",
        _ if name == "UpperExp" => "{:E}",
        _ if name == "UpperHex" => "{:X}",
        _ => unimplemented!(),
    }
}
