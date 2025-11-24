use std::{slice, vec};

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use syn::ext::IdentExt;
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::Token;

use crate::usage::{
    self, IdentRefSet, IdentSet, LifetimeRefSet, LifetimeSet, UsesLifetimes, UsesTypeParams,
};
use crate::{Error, FromField, FromVariant, Result};

/// A struct or enum body.
///
/// `V` is the type which receives any encountered variants, and `F` receives struct fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Data<V, F> {
    Enum(Vec<V>),
    Struct(Fields<F>),
}

impl<V, F> Data<V, F> {
    /// Creates an empty body of the same shape as the passed-in body.
    ///
    /// # Panics
    /// This function will panic if passed `syn::Data::Union`.
    pub fn empty_from(src: &syn::Data) -> Self {
        match *src {
            syn::Data::Enum(_) => Data::Enum(vec![]),
            syn::Data::Struct(ref vd) => Data::Struct(Fields::empty_from(&vd.fields)),
            syn::Data::Union(_) => panic!("Unions are not supported"),
        }
    }

    /// Creates an empty body of the same shape as the passed-in body.
    ///
    /// `darling` does not support unions; calling this function with a union body will return an error.
    pub fn try_empty_from(src: &syn::Data) -> Result<Self> {
        match *src {
            syn::Data::Enum(_) => Ok(Data::Enum(vec![])),
            syn::Data::Struct(ref vd) => Ok(Data::Struct(Fields::empty_from(&vd.fields))),
            // This deliberately doesn't set a span on the error message, as the error is most useful if
            // applied to the call site of the offending macro. Given that the message is very generic,
            // putting it on the union keyword ends up being confusing.
            syn::Data::Union(_) => Err(Error::custom("Unions are not supported")),
        }
    }

    /// Creates a new `Data<&'a V, &'a F>` instance from `Data<V, F>`.
    pub fn as_ref(&self) -> Data<&V, &F> {
        match *self {
            Data::Enum(ref variants) => Data::Enum(variants.iter().collect()),
            Data::Struct(ref data) => Data::Struct(data.as_ref()),
        }
    }

    /// Applies a function `V -> U` on enum variants, if this is an enum.
    pub fn map_enum_variants<T, U>(self, map: T) -> Data<U, F>
    where
        T: FnMut(V) -> U,
    {
        match self {
            Data::Enum(v) => Data::Enum(v.into_iter().map(map).collect()),
            Data::Struct(f) => Data::Struct(f),
        }
    }

    /// Applies a function `F -> U` on struct fields, if this is a struct.
    pub fn map_struct_fields<T, U>(self, map: T) -> Data<V, U>
    where
        T: FnMut(F) -> U,
    {
        match self {
            Data::Enum(v) => Data::Enum(v),
            Data::Struct(f) => Data::Struct(f.map(map)),
        }
    }

    /// Applies a function to the `Fields` if this is a struct.
    pub fn map_struct<T, U>(self, mut map: T) -> Data<V, U>
    where
        T: FnMut(Fields<F>) -> Fields<U>,
    {
        match self {
            Data::Enum(v) => Data::Enum(v),
            Data::Struct(f) => Data::Struct(map(f)),
        }
    }

    /// Consumes the `Data`, returning `Fields<F>` if it was a struct.
    pub fn take_struct(self) -> Option<Fields<F>> {
        match self {
            Data::Enum(_) => None,
            Data::Struct(f) => Some(f),
        }
    }

    /// Consumes the `Data`, returning `Vec<V>` if it was an enum.
    pub fn take_enum(self) -> Option<Vec<V>> {
        match self {
            Data::Enum(v) => Some(v),
            Data::Struct(_) => None,
        }
    }

    /// Returns `true` if this instance is `Data::Enum`.
    pub fn is_enum(&self) -> bool {
        match *self {
            Data::Enum(_) => true,
            Data::Struct(_) => false,
        }
    }

    /// Returns `true` if this instance is `Data::Struct`.
    pub fn is_struct(&self) -> bool {
        !self.is_enum()
    }
}

impl<V: FromVariant, F: FromField> Data<V, F> {
    /// Attempt to convert from a `syn::Data` instance.
    pub fn try_from(body: &syn::Data) -> Result<Self> {
        match *body {
            syn::Data::Enum(ref data) => {
                let mut errors = Error::accumulator();
                let items = data
                    .variants
                    .iter()
                    .filter_map(|v| errors.handle(FromVariant::from_variant(v)))
                    .collect();

                errors.finish_with(Data::Enum(items))
            }
            syn::Data::Struct(ref data) => Ok(Data::Struct(Fields::try_from(&data.fields)?)),
            // This deliberately doesn't set a span on the error message, as the error is most useful if
            // applied to the call site of the offending macro. Given that the message is very generic,
            // putting it on the union keyword ends up being confusing.
            syn::Data::Union(_) => Err(Error::custom("Unions are not supported")),
        }
    }
}

impl<V: UsesTypeParams, F: UsesTypeParams> UsesTypeParams for Data<V, F> {
    fn uses_type_params<'a>(
        &self,
        options: &usage::Options,
        type_set: &'a IdentSet,
    ) -> IdentRefSet<'a> {
        match *self {
            Data::Struct(ref v) => v.uses_type_params(options, type_set),
            Data::Enum(ref v) => v.uses_type_params(options, type_set),
        }
    }
}

impl<V: UsesLifetimes, F: UsesLifetimes> UsesLifetimes for Data<V, F> {
    fn uses_lifetimes<'a>(
        &self,
        options: &usage::Options,
        lifetimes: &'a LifetimeSet,
    ) -> LifetimeRefSet<'a> {
        match *self {
            Data::Struct(ref v) => v.uses_lifetimes(options, lifetimes),
            Data::Enum(ref v) => v.uses_lifetimes(options, lifetimes),
        }
    }
}

/// Equivalent to `syn::Fields`, but replaces the AST element with a generic.
#[derive(Debug, Clone)]
pub struct Fields<T> {
    pub style: Style,
    pub fields: Vec<T>,
    span: Option<Span>,
    __nonexhaustive: (),
}

impl<T> Fields<T> {
    /// Creates a new [`Fields`] struct.
    pub fn new(style: Style, fields: Vec<T>) -> Self {
        Self {
            style,
            fields,
            span: None,
            __nonexhaustive: (),
        }
    }

    /// Adds a [`Span`] to [`Fields`].
    pub fn with_span(mut self, span: Span) -> Self {
        if self.span.is_none() {
            self.span = Some(span);
        }
        self
    }

    pub fn empty_from(vd: &syn::Fields) -> Self {
        Self::new(vd.into(), Vec::new())
    }

    /// Splits the `Fields` into its style and fields for further processing.
    /// Returns an empty `Vec` for `Unit` data.
    pub fn split(self) -> (Style, Vec<T>) {
        (self.style, self.fields)
    }

    /// Returns true if this variant's data makes it a newtype.
    pub fn is_newtype(&self) -> bool {
        self.style == Style::Tuple && self.len() == 1
    }

    pub fn is_unit(&self) -> bool {
        self.style.is_unit()
    }

    pub fn is_tuple(&self) -> bool {
        self.style.is_tuple()
    }

    pub fn is_struct(&self) -> bool {
        self.style.is_struct()
    }

    pub fn as_ref(&self) -> Fields<&T> {
        Fields {
            style: self.style,
            fields: self.fields.iter().collect(),
            span: self.span,
            __nonexhaustive: (),
        }
    }

    pub fn map<F, U>(self, map: F) -> Fields<U>
    where
        F: FnMut(T) -> U,
    {
        Fields {
            style: self.style,
            fields: self.fields.into_iter().map(map).collect(),
            span: self.span,
            __nonexhaustive: (),
        }
    }

    pub fn iter(&self) -> slice::Iter<'_, T> {
        self.fields.iter()
    }

    /// Returns the number of fields in the structure.
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Returns `true` if the `Fields` contains no fields.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

impl<F: FromField> Fields<F> {
    pub fn try_from(fields: &syn::Fields) -> Result<Self> {
        let mut errors = Error::accumulator();
        let items = {
            match &fields {
                syn::Fields::Named(fields) => fields
                    .named
                    .iter()
                    .filter_map(|field| {
                        errors.handle(FromField::from_field(field).map_err(|err| {
                            // There should always be an ident here, since this is a collection
                            // of named fields, but `syn` doesn't prevent someone from manually
                            // constructing an invalid collection so a guard is still warranted.
                            if let Some(ident) = &field.ident {
                                err.at(ident)
                            } else {
                                err
                            }
                        }))
                    })
                    .collect(),
                syn::Fields::Unnamed(fields) => fields
                    .unnamed
                    .iter()
                    .filter_map(|field| errors.handle(FromField::from_field(field)))
                    .collect(),
                syn::Fields::Unit => vec![],
            }
        };

        errors.finish()?;

        Ok(Self::new(fields.into(), items).with_span(fields.span()))
    }
}

impl<T: ToTokens> ToTokens for Fields<T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let fields = &self.fields;
        // An unknown Span should be `Span::call_site()`;
        // https://docs.rs/syn/1.0.12/syn/spanned/trait.Spanned.html#tymethod.span
        let span = self.span.unwrap_or_else(Span::call_site);

        match self.style {
            Style::Struct => {
                let trailing_comma = {
                    if fields.is_empty() {
                        quote!()
                    } else {
                        quote!(,)
                    }
                };

                tokens.extend(quote_spanned![span => { #(#fields),* #trailing_comma }]);
            }
            Style::Tuple => {
                tokens.extend(quote_spanned![span => ( #(#fields),* )]);
            }
            Style::Unit => {}
        }
    }
}

impl<T: PartialEq> PartialEq for Fields<T> {
    fn eq(&self, other: &Self) -> bool {
        self.style == other.style && self.fields == other.fields
    }
}

impl<T: Eq> Eq for Fields<T> {}

impl<T> IntoIterator for Fields<T> {
    type Item = T;
    type IntoIter = vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl<T> From<Style> for Fields<T> {
    fn from(style: Style) -> Self {
        Self::new(style, Vec::new())
    }
}

impl<T, U: Into<Vec<T>>> From<(Style, U)> for Fields<T> {
    fn from((style, fields): (Style, U)) -> Self {
        style.with_fields(fields)
    }
}

impl<T: UsesTypeParams> UsesTypeParams for Fields<T> {
    fn uses_type_params<'a>(
        &self,
        options: &usage::Options,
        type_set: &'a IdentSet,
    ) -> IdentRefSet<'a> {
        self.fields.uses_type_params(options, type_set)
    }
}

impl<T: UsesLifetimes> UsesLifetimes for Fields<T> {
    fn uses_lifetimes<'a>(
        &self,
        options: &usage::Options,
        lifetimes: &'a LifetimeSet,
    ) -> LifetimeRefSet<'a> {
        self.fields.uses_lifetimes(options, lifetimes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Tuple,
    Struct,
    Unit,
}

impl Style {
    pub fn is_unit(self) -> bool {
        self == Style::Unit
    }

    pub fn is_tuple(self) -> bool {
        self == Style::Tuple
    }

    pub fn is_struct(self) -> bool {
        self == Style::Struct
    }

    /// Creates a new `Fields` of the specified style with the passed-in fields.
    fn with_fields<T, U: Into<Vec<T>>>(self, fields: U) -> Fields<T> {
        Fields::new(self, fields.into())
    }
}

impl From<syn::Fields> for Style {
    fn from(vd: syn::Fields) -> Self {
        (&vd).into()
    }
}

impl From<&syn::Fields> for Style {
    fn from(vd: &syn::Fields) -> Self {
        match *vd {
            syn::Fields::Named(_) => Style::Struct,
            syn::Fields::Unnamed(_) => Style::Tuple,
            syn::Fields::Unit => Style::Unit,
        }
    }
}

#[derive(Debug, Clone)]
pub enum NestedMeta {
    Meta(syn::Meta),
    Lit(syn::Lit),
}

impl NestedMeta {
    pub fn parse_meta_list(tokens: TokenStream) -> syn::Result<Vec<Self>> {
        syn::punctuated::Punctuated::<NestedMeta, Token![,]>::parse_terminated
            .parse2(tokens)
            .map(|punctuated| punctuated.into_iter().collect())
    }
}

impl syn::parse::Parse for NestedMeta {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(syn::Lit) && !(input.peek(syn::LitBool) && input.peek2(Token![=])) {
            input.parse().map(NestedMeta::Lit)
        } else if input.peek(syn::Ident::peek_any)
            || input.peek(Token![::]) && input.peek3(syn::Ident::peek_any)
        {
            input.parse().map(NestedMeta::Meta)
        } else {
            Err(input.error("expected identifier or literal"))
        }
    }
}

impl ToTokens for NestedMeta {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            NestedMeta::Meta(meta) => meta.to_tokens(tokens),
            NestedMeta::Lit(lit) => lit.to_tokens(tokens),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // it is not possible to directly convert a TokenStream into syn::Fields, so you have
    // to convert the TokenStream into DeriveInput first and then pass the syn::Fields to
    // Fields::try_from.
    fn token_stream_to_fields(input: TokenStream) -> Fields<syn::Field> {
        Fields::try_from(&{
            if let syn::Data::Struct(s) = syn::parse2::<syn::DeriveInput>(input).unwrap().data {
                s.fields
            } else {
                panic!();
            }
        })
        .unwrap()
    }

    #[test]
    fn test_style_eq() {
        // `Fields` implements `Eq` manually, so it has to be ensured, that all fields of `Fields`
        // implement `Eq`, this test would fail, if someone accidentally removed the Eq
        // implementation from `Style`.
        struct _AssertEq
        where
            Style: Eq;
    }

    #[test]
    fn test_fields_to_tokens_struct() {
        let reference = quote!(
            {
                executable: String,
                args: Vec<String>,
                env: Vec<String>,
                index: usize,
                optional: Option<String>,
                current_dir: String,
            }
        );
        let input = quote!(
            struct ExampleTest #reference
        );

        let fields = token_stream_to_fields(input);

        let mut result = quote!();
        fields.to_tokens(&mut result);
        assert_eq!(result.to_string(), reference.to_string());
    }

    #[test]
    fn test_fields_to_tokens_tuple() {
        let reference = quote!((u64, usize, &'a T));
        let input = quote!(
            struct ExampleTest #reference;
        );

        let fields = token_stream_to_fields(input);

        let mut result = quote!();
        fields.to_tokens(&mut result);
        assert_eq!(result.to_string(), reference.to_string());
    }
}
