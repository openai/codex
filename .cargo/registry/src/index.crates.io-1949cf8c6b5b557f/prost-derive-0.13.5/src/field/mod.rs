mod group;
mod map;
mod message;
mod oneof;
mod scalar;

use std::fmt;
use std::slice;

use anyhow::{bail, Error};
use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, ExprLit, Lit, LitBool, LitInt, Meta, MetaNameValue, Token};

#[derive(Clone)]
pub enum Field {
    /// A scalar field.
    Scalar(scalar::Field),
    /// A message field.
    Message(message::Field),
    /// A map field.
    Map(map::Field),
    /// A oneof field.
    Oneof(oneof::Field),
    /// A group field.
    Group(group::Field),
}

impl Field {
    /// Creates a new `Field` from an iterator of field attributes.
    ///
    /// If the meta items are invalid, an error will be returned.
    /// If the field should be ignored, `None` is returned.
    pub fn new(attrs: Vec<Attribute>, inferred_tag: Option<u32>) -> Result<Option<Field>, Error> {
        let attrs = prost_attrs(attrs)?;

        // TODO: check for ignore attribute.

        let field = if let Some(field) = scalar::Field::new(&attrs, inferred_tag)? {
            Field::Scalar(field)
        } else if let Some(field) = message::Field::new(&attrs, inferred_tag)? {
            Field::Message(field)
        } else if let Some(field) = map::Field::new(&attrs, inferred_tag)? {
            Field::Map(field)
        } else if let Some(field) = oneof::Field::new(&attrs)? {
            Field::Oneof(field)
        } else if let Some(field) = group::Field::new(&attrs, inferred_tag)? {
            Field::Group(field)
        } else {
            bail!("no type attribute");
        };

        Ok(Some(field))
    }

    /// Creates a new oneof `Field` from an iterator of field attributes.
    ///
    /// If the meta items are invalid, an error will be returned.
    /// If the field should be ignored, `None` is returned.
    pub fn new_oneof(attrs: Vec<Attribute>) -> Result<Option<Field>, Error> {
        let attrs = prost_attrs(attrs)?;

        // TODO: check for ignore attribute.

        let field = if let Some(field) = scalar::Field::new_oneof(&attrs)? {
            Field::Scalar(field)
        } else if let Some(field) = message::Field::new_oneof(&attrs)? {
            Field::Message(field)
        } else if let Some(field) = map::Field::new_oneof(&attrs)? {
            Field::Map(field)
        } else if let Some(field) = group::Field::new_oneof(&attrs)? {
            Field::Group(field)
        } else {
            bail!("no type attribute for oneof field");
        };

        Ok(Some(field))
    }

    pub fn tags(&self) -> Vec<u32> {
        match *self {
            Field::Scalar(ref scalar) => vec![scalar.tag],
            Field::Message(ref message) => vec![message.tag],
            Field::Map(ref map) => vec![map.tag],
            Field::Oneof(ref oneof) => oneof.tags.clone(),
            Field::Group(ref group) => vec![group.tag],
        }
    }

    /// Returns a statement which encodes the field.
    pub fn encode(&self, ident: TokenStream) -> TokenStream {
        match *self {
            Field::Scalar(ref scalar) => scalar.encode(ident),
            Field::Message(ref message) => message.encode(ident),
            Field::Map(ref map) => map.encode(ident),
            Field::Oneof(ref oneof) => oneof.encode(ident),
            Field::Group(ref group) => group.encode(ident),
        }
    }

    /// Returns an expression which evaluates to the result of merging a decoded
    /// value into the field.
    pub fn merge(&self, ident: TokenStream) -> TokenStream {
        match *self {
            Field::Scalar(ref scalar) => scalar.merge(ident),
            Field::Message(ref message) => message.merge(ident),
            Field::Map(ref map) => map.merge(ident),
            Field::Oneof(ref oneof) => oneof.merge(ident),
            Field::Group(ref group) => group.merge(ident),
        }
    }

    /// Returns an expression which evaluates to the encoded length of the field.
    pub fn encoded_len(&self, ident: TokenStream) -> TokenStream {
        match *self {
            Field::Scalar(ref scalar) => scalar.encoded_len(ident),
            Field::Map(ref map) => map.encoded_len(ident),
            Field::Message(ref msg) => msg.encoded_len(ident),
            Field::Oneof(ref oneof) => oneof.encoded_len(ident),
            Field::Group(ref group) => group.encoded_len(ident),
        }
    }

    /// Returns a statement which clears the field.
    pub fn clear(&self, ident: TokenStream) -> TokenStream {
        match *self {
            Field::Scalar(ref scalar) => scalar.clear(ident),
            Field::Message(ref message) => message.clear(ident),
            Field::Map(ref map) => map.clear(ident),
            Field::Oneof(ref oneof) => oneof.clear(ident),
            Field::Group(ref group) => group.clear(ident),
        }
    }

    pub fn default(&self) -> TokenStream {
        match *self {
            Field::Scalar(ref scalar) => scalar.default(),
            _ => quote!(::core::default::Default::default()),
        }
    }

    /// Produces the fragment implementing debug for the given field.
    pub fn debug(&self, ident: TokenStream) -> TokenStream {
        match *self {
            Field::Scalar(ref scalar) => {
                let wrapper = scalar.debug(quote!(ScalarWrapper));
                quote! {
                    {
                        #wrapper
                        ScalarWrapper(&#ident)
                    }
                }
            }
            Field::Map(ref map) => {
                let wrapper = map.debug(quote!(MapWrapper));
                quote! {
                    {
                        #wrapper
                        MapWrapper(&#ident)
                    }
                }
            }
            _ => quote!(&#ident),
        }
    }

    pub fn methods(&self, ident: &TokenStream) -> Option<TokenStream> {
        match *self {
            Field::Scalar(ref scalar) => scalar.methods(ident),
            Field::Map(ref map) => map.methods(ident),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Label {
    /// An optional field.
    Optional,
    /// A required field.
    Required,
    /// A repeated field.
    Repeated,
}

impl Label {
    fn as_str(self) -> &'static str {
        match self {
            Label::Optional => "optional",
            Label::Required => "required",
            Label::Repeated => "repeated",
        }
    }

    fn variants() -> slice::Iter<'static, Label> {
        const VARIANTS: &[Label] = &[Label::Optional, Label::Required, Label::Repeated];
        VARIANTS.iter()
    }

    /// Parses a string into a field label.
    /// If the string doesn't match a field label, `None` is returned.
    fn from_attr(attr: &Meta) -> Option<Label> {
        if let Meta::Path(ref path) = *attr {
            for &label in Label::variants() {
                if path.is_ident(label.as_str()) {
                    return Some(label);
                }
            }
        }
        None
    }
}

impl fmt::Debug for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Get the items belonging to the 'prost' list attribute, e.g. `#[prost(foo, bar="baz")]`.
fn prost_attrs(attrs: Vec<Attribute>) -> Result<Vec<Meta>, Error> {
    let mut result = Vec::new();
    for attr in attrs.iter() {
        if let Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("prost") {
                result.extend(
                    meta_list
                        .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?
                        .into_iter(),
                )
            }
        }
    }
    Ok(result)
}

pub fn set_option<T>(option: &mut Option<T>, value: T, message: &str) -> Result<(), Error>
where
    T: fmt::Debug,
{
    if let Some(ref existing) = *option {
        bail!("{}: {:?} and {:?}", message, existing, value);
    }
    *option = Some(value);
    Ok(())
}

pub fn set_bool(b: &mut bool, message: &str) -> Result<(), Error> {
    if *b {
        bail!("{}", message);
    } else {
        *b = true;
        Ok(())
    }
}

/// Unpacks an attribute into a (key, boolean) pair, returning the boolean value.
/// If the key doesn't match the attribute, `None` is returned.
fn bool_attr(key: &str, attr: &Meta) -> Result<Option<bool>, Error> {
    if !attr.path().is_ident(key) {
        return Ok(None);
    }
    match *attr {
        Meta::Path(..) => Ok(Some(true)),
        Meta::List(ref meta_list) => Ok(Some(meta_list.parse_args::<LitBool>()?.value())),
        Meta::NameValue(MetaNameValue {
            value:
                Expr::Lit(ExprLit {
                    lit: Lit::Str(ref lit),
                    ..
                }),
            ..
        }) => lit
            .value()
            .parse::<bool>()
            .map_err(Error::from)
            .map(Option::Some),
        Meta::NameValue(MetaNameValue {
            value:
                Expr::Lit(ExprLit {
                    lit: Lit::Bool(LitBool { value, .. }),
                    ..
                }),
            ..
        }) => Ok(Some(value)),
        _ => bail!("invalid {} attribute", key),
    }
}

/// Checks if an attribute matches a word.
fn word_attr(key: &str, attr: &Meta) -> bool {
    if let Meta::Path(ref path) = *attr {
        path.is_ident(key)
    } else {
        false
    }
}

pub(super) fn tag_attr(attr: &Meta) -> Result<Option<u32>, Error> {
    if !attr.path().is_ident("tag") {
        return Ok(None);
    }
    match *attr {
        Meta::List(ref meta_list) => Ok(Some(meta_list.parse_args::<LitInt>()?.base10_parse()?)),
        Meta::NameValue(MetaNameValue {
            value: Expr::Lit(ref expr),
            ..
        }) => match expr.lit {
            Lit::Str(ref lit) => lit
                .value()
                .parse::<u32>()
                .map_err(Error::from)
                .map(Option::Some),
            Lit::Int(ref lit) => Ok(Some(lit.base10_parse()?)),
            _ => bail!("invalid tag attribute: {:?}", attr),
        },
        _ => bail!("invalid tag attribute: {:?}", attr),
    }
}

fn tags_attr(attr: &Meta) -> Result<Option<Vec<u32>>, Error> {
    if !attr.path().is_ident("tags") {
        return Ok(None);
    }
    match *attr {
        Meta::List(ref meta_list) => Ok(Some(
            meta_list
                .parse_args_with(Punctuated::<LitInt, Token![,]>::parse_terminated)?
                .iter()
                .map(LitInt::base10_parse)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Meta::NameValue(MetaNameValue {
            value:
                Expr::Lit(ExprLit {
                    lit: Lit::Str(ref lit),
                    ..
                }),
            ..
        }) => lit
            .value()
            .split(',')
            .map(|s| s.trim().parse::<u32>().map_err(Error::from))
            .collect::<Result<Vec<u32>, _>>()
            .map(Some),
        _ => bail!("invalid tag attribute: {:?}", attr),
    }
}
