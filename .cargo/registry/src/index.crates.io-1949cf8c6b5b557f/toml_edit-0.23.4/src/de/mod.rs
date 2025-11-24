//! Deserializing TOML into Rust structures.
//!
//! This module contains all the Serde support for deserializing TOML documents into Rust structures.

use serde::de::DeserializeOwned;

mod array;
mod error;
mod key;
mod table;
mod table_enum;
mod value;

use array::ArrayDeserializer;
use key::KeyDeserializer;
use table::TableDeserializer;
use table_enum::TableEnumDeserializer;
use toml_datetime::de::DatetimeDeserializer;

pub use error::Error;
pub use value::ValueDeserializer;

/// Deserializes a string into a type.
///
/// This function will attempt to interpret `s` as a TOML document and
/// deserialize `T` from the document.
///
/// To deserializes TOML values, instead of documents, see [`ValueDeserializer`].
///
/// # Examples
///
/// ```
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
/// let config: Config = toml_edit::de::from_str(r#"
///     title = 'TOML Example'
///
///     [owner]
///     name = 'Lisa'
/// "#).unwrap();
///
/// assert_eq!(config.title, "TOML Example");
/// assert_eq!(config.owner.name, "Lisa");
/// ```
#[cfg(feature = "parse")]
pub fn from_str<T>(s: &'_ str) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let de = Deserializer::parse(s)?;
    T::deserialize(de)
}

/// Deserializes bytes into a type.
///
/// This function will attempt to interpret `s` as a TOML document and
/// deserialize `T` from the document.
///
/// To deserializes TOML values, instead of documents, see [`ValueDeserializer`].
#[cfg(feature = "parse")]
pub fn from_slice<T>(s: &'_ [u8]) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let s = std::str::from_utf8(s).map_err(|e| Error::custom(e, None))?;
    from_str(s)
}

/// Convert a [`DocumentMut`][crate::DocumentMut] into `T`.
pub fn from_document<T>(d: impl Into<Deserializer>) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let deserializer = d.into();
    T::deserialize(deserializer)
}

/// Deserialization for TOML [documents][crate::DocumentMut].
pub struct Deserializer<S = String> {
    root: crate::Item,
    raw: Option<S>,
}

#[cfg(feature = "parse")]
impl<S: AsRef<str>> Deserializer<S> {
    /// Parse a TOML document
    pub fn parse(raw: S) -> Result<Self, Error> {
        crate::Document::parse(raw)
            .map(Self::from)
            .map_err(Into::into)
    }
}

impl From<crate::DocumentMut> for Deserializer {
    fn from(doc: crate::DocumentMut) -> Self {
        let crate::DocumentMut { root, .. } = doc;
        Self { root, raw: None }
    }
}

impl<S> From<crate::Document<S>> for Deserializer<S> {
    fn from(doc: crate::Document<S>) -> Self {
        let crate::Document { root, raw, .. } = doc;
        let raw = Some(raw);
        Self { root, raw }
    }
}

#[cfg(feature = "parse")]
impl std::str::FromStr for Deserializer {
    type Err = Error;

    /// Parses a document from a &str
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let doc: crate::Document<_> = s.parse().map_err(Error::from)?;
        Ok(Self::from(doc))
    }
}

impl<'de, S: AsRef<str>> serde::Deserializer<'de> for Deserializer<S> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let raw = self.raw;
        ValueDeserializer::new(self.root)
            .deserialize_any(visitor)
            .map_err(|mut e: Self::Error| {
                let raw = raw.as_ref().map(|r| r.as_ref());
                e.set_input(raw);
                e
            })
    }

    // `None` is interpreted as a missing field so be sure to implement `Some`
    // as a present field.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let raw = self.raw;
        ValueDeserializer::new(self.root)
            .deserialize_option(visitor)
            .map_err(|mut e: Self::Error| {
                let raw = raw.as_ref().map(|r| r.as_ref());
                e.set_input(raw);
                e
            })
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let raw = self.raw;
        ValueDeserializer::new(self.root)
            .deserialize_newtype_struct(name, visitor)
            .map_err(|mut e: Self::Error| {
                let raw = raw.as_ref().map(|r| r.as_ref());
                e.set_input(raw);
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
        let raw = self.raw;
        ValueDeserializer::new(self.root)
            .deserialize_struct(name, fields, visitor)
            .map_err(|mut e: Self::Error| {
                let raw = raw.as_ref().map(|r| r.as_ref());
                e.set_input(raw);
                e
            })
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
        let raw = self.raw;
        ValueDeserializer::new(self.root)
            .deserialize_enum(name, variants, visitor)
            .map_err(|mut e: Self::Error| {
                let raw = raw.as_ref().map(|r| r.as_ref());
                e.set_input(raw);
                e
            })
    }

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map unit
        ignored_any unit_struct tuple_struct tuple identifier
    }
}

impl serde::de::IntoDeserializer<'_, Error> for Deserializer {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl serde::de::IntoDeserializer<'_, Error> for crate::DocumentMut {
    type Deserializer = Deserializer;

    fn into_deserializer(self) -> Self::Deserializer {
        Deserializer::from(self)
    }
}

impl serde::de::IntoDeserializer<'_, Error> for crate::Document<String> {
    type Deserializer = Deserializer;

    fn into_deserializer(self) -> Self::Deserializer {
        Deserializer::from(self)
    }
}
