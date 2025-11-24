//! Serializing Rust structures into TOML.
//!
//! This module contains all the Serde support for serializing Rust structures
//! into TOML documents (as strings). Note that some top-level functions here
//! are also provided at the top of the crate.

mod array;
mod array_of_tables;
mod buffer;
mod map;
mod strategy;

use toml_writer::TomlWrite as _;

use super::style;
use super::value;
use super::Error;
use crate::alloc_prelude::*;
use buffer::Table;
use strategy::SerializationStrategy;

pub use buffer::Buffer;

/// Serialization for TOML documents.
///
/// This structure implements serialization support for TOML to serialize an
/// arbitrary type to TOML. Note that the TOML format does not support all
/// datatypes in Rust, such as enums, tuples, and tuple structs. These types
/// will generate an error when serialized.
///
/// Currently a serializer always writes its output to an in-memory `String`,
/// which is passed in when creating the serializer itself.
///
/// To serialize TOML values, instead of documents, see
/// [`ValueSerializer`][super::value::ValueSerializer].
pub struct Serializer<'d> {
    buf: &'d mut Buffer,
    style: style::Style,
    table: Table,
}

impl<'d> Serializer<'d> {
    /// Creates a new serializer which will emit TOML into the buffer provided.
    ///
    /// The serializer can then be used to serialize a type after which the data
    /// will be present in `buf`.
    pub fn new(buf: &'d mut Buffer) -> Self {
        let table = buf.root_table();
        Self {
            buf,
            style: Default::default(),
            table,
        }
    }

    /// Apply a default "pretty" policy to the document
    ///
    /// For greater customization, instead serialize to a
    /// [`toml_edit::DocumentMut`](https://docs.rs/toml_edit/latest/toml_edit/struct.DocumentMut.html).
    pub fn pretty(buf: &'d mut Buffer) -> Self {
        let mut ser = Serializer::new(buf);
        ser.style.multiline_array = true;
        ser
    }

    pub(crate) fn with_table(buf: &'d mut Buffer, table: Table, style: style::Style) -> Self {
        Self { buf, style, table }
    }

    fn end(self) -> Result<&'d mut Buffer, Error> {
        self.buf.push(self.table);
        Ok(self.buf)
    }
}

impl<'d> serde::ser::Serializer for Serializer<'d> {
    type Ok = &'d mut Buffer;
    type Error = Error;
    type SerializeSeq = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = array::SerializeDocumentTupleVariant<'d>;
    type SerializeMap = map::SerializeDocumentTable<'d>;
    type SerializeStruct = map::SerializeDocumentTable<'d>;
    type SerializeStructVariant = map::SerializeDocumentTable<'d>;

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("bool")))
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("i8")))
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("i16")))
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("i32")))
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("i64")))
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("u8")))
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("u16")))
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("u32")))
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("u64")))
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("f32")))
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("f64")))
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("char")))
    }

    fn serialize_str(self, _v: &str) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("str")))
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("bytes")))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_none())
    }

    fn serialize_some<T>(self, v: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        v.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some("unit")))
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some(name)))
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Error::unsupported_type(Some(name)))
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        v: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        v.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        mut self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        match SerializationStrategy::from(value) {
            SerializationStrategy::Value | SerializationStrategy::ArrayOfTables => {
                let dst = self.table.body_mut();

                dst.key(variant)?;
                dst.space()?;
                dst.keyval_sep()?;
                dst.space()?;
                let value_serializer = value::ValueSerializer::with_style(dst, self.style);
                let dst = value.serialize(value_serializer)?;
                dst.newline()?;
            }
            SerializationStrategy::Table | SerializationStrategy::Unknown => {
                let child = self.buf.child_table(&mut self.table, variant.to_owned());
                let value_serializer = Serializer::with_table(self.buf, child, self.style);
                value.serialize(value_serializer)?;
            }
            SerializationStrategy::Skip => {
                // silently drop these key-value pairs
            }
        }

        self.end()
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Error::unsupported_type(Some("array")))
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        array::SerializeDocumentTupleVariant::tuple(self.buf, self.table, variant, len, self.style)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        map::SerializeDocumentTable::map(self.buf, self.table, self.style)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        mut self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        let child = self.buf.child_table(&mut self.table, variant.to_owned());
        self.buf.push(self.table);
        map::SerializeDocumentTable::map(self.buf, child, self.style)
    }
}
