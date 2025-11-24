use serde::de::IntoDeserializer as _;
use serde_spanned::Spanned;

use super::ArrayDeserializer;
use super::DatetimeDeserializer;
use super::TableDeserializer;
use crate::alloc_prelude::*;
use crate::de::DeString;
use crate::de::DeTable;
use crate::de::DeValue;
use crate::de::Error;

/// Deserialization implementation for TOML [values][crate::Value].
///
/// # Example
///
/// ```
/// # #[cfg(feature = "parse")] {
/// # #[cfg(feature = "display")] {
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Config {
///     title: String,
///     owner: Owner,
/// }
///
/// #[derive(Deserialize)]
/// struct Owner {
///     name: String,
/// }
///
/// let value = r#"{ title = 'TOML Example', owner = { name = 'Lisa' } }"#;
/// let deserializer = toml::de::ValueDeserializer::parse(value).unwrap();
/// let config = Config::deserialize(deserializer).unwrap();
/// assert_eq!(config.title, "TOML Example");
/// assert_eq!(config.owner.name, "Lisa");
/// # }
/// # }
/// ```
pub struct ValueDeserializer<'i> {
    span: core::ops::Range<usize>,
    input: DeValue<'i>,
    validate_struct_keys: bool,
}

impl<'i> ValueDeserializer<'i> {
    /// Parse a TOML value
    pub fn parse(raw: &'i str) -> Result<Self, Error> {
        let input = DeValue::parse(raw)?;
        let span = input.span();
        let input = input.into_inner();
        Ok(Self::with_parts(input, span))
    }

    /// Deprecated, replaced with [`ValueDeserializer::parse`]
    #[deprecated(since = "0.9.0", note = "replaced with `ValueDeserializer::parse`")]
    pub fn new(raw: &'i str) -> Result<Self, Error> {
        Self::parse(raw)
    }

    pub(crate) fn with_parts(input: DeValue<'i>, span: core::ops::Range<usize>) -> Self {
        Self {
            input,
            span,
            validate_struct_keys: false,
        }
    }

    pub(crate) fn with_struct_key_validation(mut self) -> Self {
        self.validate_struct_keys = true;
        self
    }
}

impl<'i> From<Spanned<DeValue<'i>>> for ValueDeserializer<'i> {
    fn from(root: Spanned<DeValue<'i>>) -> Self {
        let span = root.span();
        let root = root.into_inner();
        Self::with_parts(root, span)
    }
}

impl<'de> serde::Deserializer<'de> for ValueDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let span = self.span.clone();
        match self.input {
            DeValue::String(DeString::Owned(v)) => visitor.visit_string(v),
            DeValue::String(DeString::Borrowed(v)) => visitor.visit_str(v),
            DeValue::Integer(v) => {
                if let Some(v) = v.to_i64() {
                    visitor.visit_i64(v)
                } else if let Some(v) = v.to_u64() {
                    visitor.visit_u64(v)
                } else if let Some(v) = v.to_i128() {
                    visitor.visit_i128(v)
                } else if let Some(v) = v.to_u128() {
                    visitor.visit_u128(v)
                } else {
                    Err(Error::custom("integer number overflowed", None))
                }
            }
            DeValue::Float(v) => {
                if let Some(v) = v.to_f64() {
                    visitor.visit_f64(v)
                } else {
                    Err(Error::custom("floating-point number overflowed", None))
                }
            }
            DeValue::Boolean(v) => visitor.visit_bool(v),
            DeValue::Datetime(v) => visitor.visit_map(DatetimeDeserializer::new(v)),
            DeValue::Array(v) => ArrayDeserializer::new(v, span.clone()).deserialize_any(visitor),
            DeValue::Table(v) => TableDeserializer::new(v, span.clone()).deserialize_any(visitor),
        }
        .map_err(|mut e: Self::Error| {
            if e.span().is_none() {
                e.set_span(Some(span));
            }
            e
        })
    }

    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    // `None` is interpreted as a missing field so be sure to implement `Some`
    // as a present field.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let span = self.span.clone();
        visitor.visit_some(self).map_err(|mut e: Self::Error| {
            if e.span().is_none() {
                e.set_span(Some(span));
            }
            e
        })
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let span = self.span.clone();
        visitor
            .visit_newtype_struct(self)
            .map_err(|mut e: Self::Error| {
                if e.span().is_none() {
                    e.set_span(Some(span));
                }
                e
            })
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if serde_spanned::de::is_spanned(name) {
            let span = self.span.clone();
            return visitor.visit_map(super::SpannedDeserializer::new(self, span));
        }

        if toml_datetime::de::is_datetime(name) {
            let span = self.span.clone();
            if let DeValue::Datetime(d) = self.input {
                return visitor.visit_map(DatetimeDeserializer::new(d)).map_err(
                    |mut e: Self::Error| {
                        if e.span().is_none() {
                            e.set_span(Some(span));
                        }
                        e
                    },
                );
            }
        }

        if self.validate_struct_keys {
            let span = self.span.clone();
            match &self.input {
                DeValue::Table(values) => validate_struct_keys(values, fields),
                _ => Ok(()),
            }
            .map_err(|mut e: Self::Error| {
                if e.span().is_none() {
                    e.set_span(Some(span));
                }
                e
            })?;
        }

        self.deserialize_any(visitor)
    }

    // Called when the type to deserialize is an enum, as opposed to a field in the type.
    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let span = self.span.clone();
        match self.input {
            DeValue::String(v) => visitor.visit_enum(v.into_deserializer()),
            DeValue::Table(v) => {
                TableDeserializer::new(v, span.clone()).deserialize_enum(name, variants, visitor)
            }
            _ => Err(Error::custom("wanted string or table", Some(span.clone()))),
        }
        .map_err(|mut e: Self::Error| {
            if e.span().is_none() {
                e.set_span(Some(span));
            }
            e
        })
    }

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map unit
        ignored_any unit_struct tuple_struct tuple identifier
    }
}

impl<'de> serde::de::IntoDeserializer<'de, Error> for ValueDeserializer<'de> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> serde::de::IntoDeserializer<'de, Error> for Spanned<DeValue<'de>> {
    type Deserializer = ValueDeserializer<'de>;

    fn into_deserializer(self) -> Self::Deserializer {
        ValueDeserializer::from(self)
    }
}

pub(crate) fn validate_struct_keys(
    table: &DeTable<'_>,
    fields: &'static [&'static str],
) -> Result<(), Error> {
    let extra_fields = table
        .keys()
        .filter_map(|key| {
            if !fields.contains(&key.get_ref().as_ref()) {
                Some(key.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if extra_fields.is_empty() {
        Ok(())
    } else {
        Err(Error::custom(
            format!(
                "unexpected keys in table: {}, available keys: {}",
                extra_fields
                    .iter()
                    .map(|k| k.get_ref().as_ref())
                    .collect::<Vec<_>>()
                    .join(", "),
                fields.join(", "),
            ),
            Some(extra_fields[0].span()),
        ))
    }
}
