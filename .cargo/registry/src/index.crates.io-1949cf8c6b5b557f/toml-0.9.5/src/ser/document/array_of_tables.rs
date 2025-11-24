use super::style::Style;
use super::Buffer;
use super::Error;
use super::Serializer;
use super::Table;
use crate::alloc_prelude::*;

pub(crate) struct ArrayOfTablesSerializer<'d> {
    buf: &'d mut Buffer,
    parent: Table,
    key: String,
    style: Style,
}

impl<'d> ArrayOfTablesSerializer<'d> {
    /// Creates a new serializer which will emit TOML into the buffer provided.
    ///
    /// The serializer can then be used to serialize a type after which the data
    /// will be present in `dst`.
    pub(crate) fn new(buf: &'d mut Buffer, parent: Table, key: String, style: Style) -> Self {
        Self {
            buf,
            parent,
            key,
            style,
        }
    }
}

impl<'d> serde::ser::Serializer for ArrayOfTablesSerializer<'d> {
    type Ok = &'d mut Buffer;
    type Error = Error;
    type SerializeSeq = SerializeArrayOfTablesSerializer<'d>;
    type SerializeTuple = SerializeArrayOfTablesSerializer<'d>;
    type SerializeTupleStruct = SerializeArrayOfTablesSerializer<'d>;
    type SerializeTupleVariant = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = serde::ser::Impossible<Self::Ok, Self::Error>;

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
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        Err(Error::unsupported_type(Some(variant)))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SerializeArrayOfTablesSerializer::seq(
            self.buf,
            self.parent,
            self.key,
            self.style,
        ))
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
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::unsupported_type(Some(variant)))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::unsupported_type(Some("map")))
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(Error::unsupported_type(Some(name)))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::unsupported_type(Some(variant)))
    }
}

#[doc(hidden)]
pub(crate) struct SerializeArrayOfTablesSerializer<'d> {
    buf: &'d mut Buffer,
    parent: Table,
    key: String,
    style: Style,
}

impl<'d> SerializeArrayOfTablesSerializer<'d> {
    pub(crate) fn seq(buf: &'d mut Buffer, parent: Table, key: String, style: Style) -> Self {
        Self {
            buf,
            parent,
            key,
            style,
        }
    }

    fn end(self) -> Result<&'d mut Buffer, Error> {
        Ok(self.buf)
    }
}

impl<'d> serde::ser::SerializeSeq for SerializeArrayOfTablesSerializer<'d> {
    type Ok = &'d mut Buffer;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let child = self.buf.element_table(&mut self.parent, self.key.clone());
        let value_serializer = Serializer::with_table(self.buf, child, self.style);
        value.serialize(value_serializer)?;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.end()
    }
}

impl<'d> serde::ser::SerializeTuple for SerializeArrayOfTablesSerializer<'d> {
    type Ok = &'d mut Buffer;
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}

impl<'d> serde::ser::SerializeTupleStruct for SerializeArrayOfTablesSerializer<'d> {
    type Ok = &'d mut Buffer;
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        serde::ser::SerializeSeq::end(self)
    }
}
