use attr;
use proc_macro2;
use syn;
use syn::spanned::Spanned as SynSpanned;

#[derive(Debug)]
pub struct Input<'a> {
    pub attrs: attr::Input,
    pub body: Body<'a>,
    pub generics: &'a syn::Generics,
    pub ident: syn::Ident,
    pub span: proc_macro2::Span,
}

#[derive(Debug)]
pub enum Body<'a> {
    Enum(Vec<Variant<'a>>),
    Struct(Style, Vec<Field<'a>>),
}

#[derive(Debug)]
pub struct Variant<'a> {
    pub attrs: attr::Input,
    pub fields: Vec<Field<'a>>,
    pub ident: syn::Ident,
    pub style: Style,
}

#[derive(Debug)]
pub struct Field<'a> {
    pub attrs: attr::Field,
    pub ident: Option<syn::Ident>,
    pub ty: &'a syn::Type,
    pub span: proc_macro2::Span,
}

#[derive(Clone, Copy, Debug)]
pub enum Style {
    Struct,
    Tuple,
    Unit,
}

impl<'a> Input<'a> {
    pub fn from_ast(
        item: &'a syn::DeriveInput,
        errors: &mut proc_macro2::TokenStream,
    ) -> Result<Input<'a>, ()> {
        let attrs = attr::Input::from_ast(&item.attrs, errors)?;

        let body = match item.data {
            syn::Data::Enum(syn::DataEnum { ref variants, .. }) => {
                Body::Enum(enum_from_ast(variants, errors)?)
            }
            syn::Data::Struct(syn::DataStruct { ref fields, .. }) => {
                let (style, fields) = struct_from_ast(fields, errors)?;
                Body::Struct(style, fields)
            }
            syn::Data::Union(..) => {
                errors.extend(
                    syn::Error::new_spanned(item, "derivative does not support unions")
                        .to_compile_error(),
                );
                return Err(());
            }
        };

        Ok(Input {
            attrs,
            body,
            generics: &item.generics,
            ident: item.ident.clone(),
            span: item.span(),
        })
    }

    /// Checks whether this type is an enum with only unit variants.
    pub fn is_trivial_enum(&self) -> bool {
        match &self.body {
            Body::Enum(e) => e.iter().all(|v| v.is_unit()),
            Body::Struct(..) => false,
        }
    }
}

impl<'a> Body<'a> {
    pub fn all_fields(&self) -> Vec<&Field> {
        match *self {
            Body::Enum(ref variants) => variants
                .iter()
                .flat_map(|variant| variant.fields.iter())
                .collect(),
            Body::Struct(_, ref fields) => fields.iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match *self {
            Body::Enum(ref variants) => variants.is_empty(),
            Body::Struct(_, ref fields) => fields.is_empty(),
        }
    }
}

impl<'a> Variant<'a> {
    /// Checks whether this variant is a unit variant.
    pub fn is_unit(&self) -> bool {
        self.fields.is_empty()
    }
}

fn enum_from_ast<'a>(
    variants: &'a syn::punctuated::Punctuated<syn::Variant, syn::token::Comma>,
    errors: &mut proc_macro2::TokenStream,
) -> Result<Vec<Variant<'a>>, ()> {
    variants
        .iter()
        .map(|variant| {
            let (style, fields) = struct_from_ast(&variant.fields, errors)?;
            Ok(Variant {
                attrs: attr::Input::from_ast(&variant.attrs, errors)?,
                fields,
                ident: variant.ident.clone(),
                style,
            })
        })
        .collect()
}

fn struct_from_ast<'a>(
    fields: &'a syn::Fields,
    errors: &mut proc_macro2::TokenStream,
) -> Result<(Style, Vec<Field<'a>>), ()> {
    match *fields {
        syn::Fields::Named(ref fields) => {
            Ok((Style::Struct, fields_from_ast(&fields.named, errors)?))
        }
        syn::Fields::Unnamed(ref fields) => {
            Ok((Style::Tuple, fields_from_ast(&fields.unnamed, errors)?))
        }
        syn::Fields::Unit => Ok((Style::Unit, Vec::new())),
    }
}

fn fields_from_ast<'a>(
    fields: &'a syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    errors: &mut proc_macro2::TokenStream,
) -> Result<Vec<Field<'a>>, ()> {
    fields
        .iter()
        .map(|field| {
            Ok(Field {
                attrs: attr::Field::from_ast(field, errors)?,
                ident: field.ident.clone(),
                ty: &field.ty,
                span: field.span(),
            })
        })
        .collect()
}
