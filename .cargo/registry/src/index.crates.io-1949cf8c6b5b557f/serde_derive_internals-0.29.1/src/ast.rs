//! A Serde ast, parsed from the Syn ast and ready to generate Rust code.

use crate::internals::{attr, check, Ctxt, Derive};
use syn::punctuated::Punctuated;
use syn::Token;

/// A source data structure annotated with `#[derive(Serialize)]` and/or `#[derive(Deserialize)]`,
/// parsed into an internal representation.
pub struct Container<'a> {
    /// The struct or enum name (without generics).
    pub ident: syn::Ident,
    /// Attributes on the structure, parsed for Serde.
    pub attrs: attr::Container,
    /// The contents of the struct or enum.
    pub data: Data<'a>,
    /// Any generics on the struct or enum.
    pub generics: &'a syn::Generics,
    /// Original input.
    pub original: &'a syn::DeriveInput,
}

/// The fields of a struct or enum.
///
/// Analogous to `syn::Data`.
pub enum Data<'a> {
    Enum(Vec<Variant<'a>>),
    Struct(Style, Vec<Field<'a>>),
}

/// A variant of an enum.
pub struct Variant<'a> {
    pub ident: syn::Ident,
    pub attrs: attr::Variant,
    pub style: Style,
    pub fields: Vec<Field<'a>>,
    pub original: &'a syn::Variant,
}

/// A field of a struct.
pub struct Field<'a> {
    pub member: syn::Member,
    pub attrs: attr::Field,
    pub ty: &'a syn::Type,
    pub original: &'a syn::Field,
}

#[derive(Copy, Clone)]
pub enum Style {
    /// Named fields.
    Struct,
    /// Many unnamed fields.
    Tuple,
    /// One unnamed field.
    Newtype,
    /// No fields.
    Unit,
}

impl<'a> Container<'a> {
    /// Convert the raw Syn ast into a parsed container object, collecting errors in `cx`.
    pub fn from_ast(
        cx: &Ctxt,
        item: &'a syn::DeriveInput,
        derive: Derive,
    ) -> Option<Container<'a>> {
        let mut attrs = attr::Container::from_ast(cx, item);

        let mut data = match &item.data {
            syn::Data::Enum(data) => Data::Enum(enum_from_ast(cx, &data.variants, attrs.default())),
            syn::Data::Struct(data) => {
                let (style, fields) = struct_from_ast(cx, &data.fields, None, attrs.default());
                Data::Struct(style, fields)
            }
            syn::Data::Union(_) => {
                cx.error_spanned_by(item, "Serde does not support derive for unions");
                return None;
            }
        };

        let mut has_flatten = false;
        match &mut data {
            Data::Enum(variants) => {
                for variant in variants {
                    variant.attrs.rename_by_rules(attrs.rename_all_rules());
                    for field in &mut variant.fields {
                        if field.attrs.flatten() {
                            has_flatten = true;
                        }
                        field.attrs.rename_by_rules(
                            variant
                                .attrs
                                .rename_all_rules()
                                .or(attrs.rename_all_fields_rules()),
                        );
                    }
                }
            }
            Data::Struct(_, fields) => {
                for field in fields {
                    if field.attrs.flatten() {
                        has_flatten = true;
                    }
                    field.attrs.rename_by_rules(attrs.rename_all_rules());
                }
            }
        }

        if has_flatten {
            attrs.mark_has_flatten();
        }

        let mut item = Container {
            ident: item.ident.clone(),
            attrs,
            data,
            generics: &item.generics,
            original: item,
        };
        check::check(cx, &mut item, derive);
        Some(item)
    }
}

impl<'a> Data<'a> {
    pub fn all_fields(&'a self) -> Box<dyn Iterator<Item = &'a Field<'a>> + 'a> {
        match self {
            Data::Enum(variants) => {
                Box::new(variants.iter().flat_map(|variant| variant.fields.iter()))
            }
            Data::Struct(_, fields) => Box::new(fields.iter()),
        }
    }

    pub fn has_getter(&self) -> bool {
        self.all_fields().any(|f| f.attrs.getter().is_some())
    }
}

fn enum_from_ast<'a>(
    cx: &Ctxt,
    variants: &'a Punctuated<syn::Variant, Token![,]>,
    container_default: &attr::Default,
) -> Vec<Variant<'a>> {
    let variants: Vec<Variant> = variants
        .iter()
        .map(|variant| {
            let attrs = attr::Variant::from_ast(cx, variant);
            let (style, fields) =
                struct_from_ast(cx, &variant.fields, Some(&attrs), container_default);
            Variant {
                ident: variant.ident.clone(),
                attrs,
                style,
                fields,
                original: variant,
            }
        })
        .collect();

    let index_of_last_tagged_variant = variants
        .iter()
        .rposition(|variant| !variant.attrs.untagged());
    if let Some(index_of_last_tagged_variant) = index_of_last_tagged_variant {
        for variant in &variants[..index_of_last_tagged_variant] {
            if variant.attrs.untagged() {
                cx.error_spanned_by(&variant.ident, "all variants with the #[serde(untagged)] attribute must be placed at the end of the enum");
            }
        }
    }

    variants
}

fn struct_from_ast<'a>(
    cx: &Ctxt,
    fields: &'a syn::Fields,
    attrs: Option<&attr::Variant>,
    container_default: &attr::Default,
) -> (Style, Vec<Field<'a>>) {
    match fields {
        syn::Fields::Named(fields) => (
            Style::Struct,
            fields_from_ast(cx, &fields.named, attrs, container_default),
        ),
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => (
            Style::Newtype,
            fields_from_ast(cx, &fields.unnamed, attrs, container_default),
        ),
        syn::Fields::Unnamed(fields) => (
            Style::Tuple,
            fields_from_ast(cx, &fields.unnamed, attrs, container_default),
        ),
        syn::Fields::Unit => (Style::Unit, Vec::new()),
    }
}

fn fields_from_ast<'a>(
    cx: &Ctxt,
    fields: &'a Punctuated<syn::Field, Token![,]>,
    attrs: Option<&attr::Variant>,
    container_default: &attr::Default,
) -> Vec<Field<'a>> {
    fields
        .iter()
        .enumerate()
        .map(|(i, field)| Field {
            member: match &field.ident {
                Some(ident) => syn::Member::Named(ident.clone()),
                None => syn::Member::Unnamed(i.into()),
            },
            attrs: attr::Field::from_ast(cx, i, field, attrs, container_default),
            ty: &field.ty,
            original: field,
        })
        .collect()
}
