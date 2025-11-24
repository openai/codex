//! Deserializing TOML into Rust structures.
//!
//! This module contains all the Serde support for deserializing TOML documents
//! into Rust structures. Note that some top-level functions here are also
//! provided at the top of the crate.

mod array;
mod key;
mod table;
mod table_enum;
mod value;

pub use value::ValueDeserializer;

use crate::de::DeTable;
use crate::de::DeValue;
use crate::de::Error;
use array::ArrayDeserializer;
use key::KeyDeserializer;
use serde_spanned::de::SpannedDeserializer;
use serde_spanned::Spanned;
use table::TableDeserializer;
use table_enum::TableEnumDeserializer;
use toml_datetime::de::DatetimeDeserializer;

/// Deserialization for TOML [documents][crate::Table].
///
/// To deserializes TOML values, instead of documents, see [`ValueDeserializer`].
pub struct Deserializer<'i> {
    span: core::ops::Range<usize>,
    root: DeTable<'i>,
    raw: Option<&'i str>,
}

impl<'i> Deserializer<'i> {
    /// Parse a TOML document
    pub fn parse(raw: &'i str) -> Result<Self, Error> {
        let root = DeTable::parse(raw)?;
        let span = root.span();
        let root = root.into_inner();
        Ok(Self {
            span,
            root,
            raw: Some(raw),
        })
    }

    /// Deprecated, replaced with [`Deserializer::parse`]
    #[deprecated(since = "0.9.0", note = "replaced with `Deserializer::parse`")]
    pub fn new(raw: &'i str) -> Result<Self, Error> {
        Self::parse(raw)
    }

    fn into_table_de(self) -> ValueDeserializer<'i> {
        ValueDeserializer::with_parts(DeValue::Table(self.root), self.span)
    }
}

impl<'i> From<Spanned<DeTable<'i>>> for Deserializer<'i> {
    fn from(root: Spanned<DeTable<'i>>) -> Self {
        let span = root.span();
        let root = root.into_inner();
        Self {
            span,
            root,
            raw: None,
        }
    }
}

impl<'de> serde::Deserializer<'de> for Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        let raw = self.raw;
        self.into_table_de()
            .deserialize_any(visitor)
            .map_err(|mut e: Self::Error| {
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
        self.into_table_de()
            .deserialize_option(visitor)
            .map_err(|mut e: Self::Error| {
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
        self.into_table_de()
            .deserialize_newtype_struct(name, visitor)
            .map_err(|mut e: Self::Error| {
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
        self.into_table_de()
            .deserialize_struct(name, fields, visitor)
            .map_err(|mut e: Self::Error| {
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
        self.into_table_de()
            .deserialize_enum(name, variants, visitor)
            .map_err(|mut e: Self::Error| {
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

impl<'de> serde::de::IntoDeserializer<'de, Error> for Deserializer<'de> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

impl<'de> serde::de::IntoDeserializer<'de, Error> for Spanned<DeTable<'de>> {
    type Deserializer = Deserializer<'de>;

    fn into_deserializer(self) -> Self::Deserializer {
        Deserializer::from(self)
    }
}
