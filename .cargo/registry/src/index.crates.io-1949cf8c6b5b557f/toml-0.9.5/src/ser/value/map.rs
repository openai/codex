use core::fmt::Write as _;

use toml_writer::TomlWrite as _;

use super::array::SerializeTupleVariant;
use super::array::SerializeValueArray;
use super::key::KeySerializer;
use super::Error;
use super::Style;
use super::ValueSerializer;
use crate::alloc_prelude::*;

#[doc(hidden)]
#[allow(clippy::large_enum_variant)]
pub enum SerializeMap<'d> {
    Datetime(SerializeDatetime<'d>),
    Table(SerializeTable<'d>),
}

impl<'d> SerializeMap<'d> {
    pub(crate) fn map(dst: &'d mut String, style: Style) -> Result<Self, Error> {
        Ok(Self::Table(SerializeTable::map(dst, style)?))
    }

    pub(crate) fn struct_(
        name: &'static str,
        dst: &'d mut String,
        style: Style,
    ) -> Result<Self, Error> {
        if toml_datetime::ser::is_datetime(name) {
            Ok(Self::Datetime(SerializeDatetime::new(dst)))
        } else {
            Ok(Self::map(dst, style)?)
        }
    }
}

impl<'d> serde::ser::SerializeMap for SerializeMap<'d> {
    type Ok = &'d mut String;
    type Error = Error;

    fn serialize_key<T>(&mut self, input: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        match self {
            Self::Datetime(s) => s.serialize_key(input),
            Self::Table(s) => s.serialize_key(input),
        }
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        match self {
            Self::Datetime(s) => s.serialize_value(value),
            Self::Table(s) => s.serialize_value(value),
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            Self::Datetime(s) => s.end(),
            Self::Table(s) => s.end(),
        }
    }
}

impl<'d> serde::ser::SerializeStruct for SerializeMap<'d> {
    type Ok = &'d mut String;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        match self {
            Self::Datetime(s) => s.serialize_field(key, value),
            Self::Table(s) => s.serialize_field(key, value),
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        match self {
            Self::Datetime(s) => s.end(),
            Self::Table(s) => s.end(),
        }
    }
}

#[doc(hidden)]
pub struct SerializeDatetime<'d> {
    dst: &'d mut String,
    inner: toml_datetime::ser::DatetimeSerializer,
}

impl<'d> SerializeDatetime<'d> {
    pub(crate) fn new(dst: &'d mut String) -> Self {
        Self {
            dst,
            inner: toml_datetime::ser::DatetimeSerializer::new(),
        }
    }
}

impl<'d> serde::ser::SerializeMap for SerializeDatetime<'d> {
    type Ok = &'d mut String;
    type Error = Error;

    fn serialize_key<T>(&mut self, _input: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        unreachable!("datetimes should only be serialized as structs, not maps")
    }

    fn serialize_value<T>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        unreachable!("datetimes should only be serialized as structs, not maps")
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        unreachable!("datetimes should only be serialized as structs, not maps")
    }
}

impl<'d> serde::ser::SerializeStruct for SerializeDatetime<'d> {
    type Ok = &'d mut String;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        self.inner.serialize_field(key, value).map_err(dt_err)?;

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let value = self.inner.end().map_err(dt_err)?;
        write!(self.dst, "{value}")?;
        Ok(self.dst)
    }
}

fn dt_err(err: toml_datetime::ser::SerializerError) -> Error {
    match err {
        toml_datetime::ser::SerializerError::InvalidFormat(err) => Error::new(err),
        _ => Error::date_invalid(),
    }
}

#[doc(hidden)]
pub struct SerializeTable<'d> {
    dst: &'d mut String,
    seen_value: bool,
    key: Option<String>,
    style: Style,
}

impl<'d> SerializeTable<'d> {
    pub(crate) fn map(dst: &'d mut String, style: Style) -> Result<Self, Error> {
        dst.open_inline_table()?;
        Ok(Self {
            dst,
            seen_value: false,
            key: None,
            style,
        })
    }

    pub(crate) fn end(self) -> Result<&'d mut String, Error> {
        if self.seen_value {
            self.dst.space()?;
        }
        self.dst.close_inline_table()?;
        Ok(self.dst)
    }
}

impl<'d> serde::ser::SerializeMap for SerializeTable<'d> {
    type Ok = &'d mut String;
    type Error = Error;

    fn serialize_key<T>(&mut self, input: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let mut encoded_key = String::new();
        input.serialize(KeySerializer {
            dst: &mut encoded_key,
        })?;
        self.key = Some(encoded_key);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let encoded_key = self
            .key
            .take()
            .expect("always called after `serialize_key`");
        let mut encoded_value = String::new();
        let mut is_none = false;
        let value_serializer =
            MapValueSerializer::new(&mut encoded_value, &mut is_none, self.style);
        let res = value.serialize(value_serializer);
        match res {
            Ok(_) => {
                use core::fmt::Write as _;

                if self.seen_value {
                    self.dst.val_sep()?;
                }
                self.seen_value = true;
                self.dst.space()?;
                write!(self.dst, "{encoded_key}")?;
                self.dst.space()?;
                self.dst.keyval_sep()?;
                self.dst.space()?;
                write!(self.dst, "{encoded_value}")?;
            }
            Err(e) => {
                if !(e == Error::unsupported_none() && is_none) {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.end()
    }
}

impl<'d> serde::ser::SerializeStruct for SerializeTable<'d> {
    type Ok = &'d mut String;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let mut encoded_value = String::new();
        let mut is_none = false;
        let value_serializer =
            MapValueSerializer::new(&mut encoded_value, &mut is_none, self.style);
        let res = value.serialize(value_serializer);
        match res {
            Ok(_) => {
                use core::fmt::Write as _;

                if self.seen_value {
                    self.dst.val_sep()?;
                }
                self.seen_value = true;
                self.dst.space()?;
                self.dst.key(key)?;
                self.dst.space()?;
                self.dst.keyval_sep()?;
                self.dst.space()?;
                write!(self.dst, "{encoded_value}")?;
            }
            Err(e) => {
                if !(e == Error::unsupported_none() && is_none) {
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.end()
    }
}

pub(crate) struct MapValueSerializer<'d> {
    dst: &'d mut String,
    is_none: &'d mut bool,
    style: Style,
}

impl<'d> MapValueSerializer<'d> {
    pub(crate) fn new(dst: &'d mut String, is_none: &'d mut bool, style: Style) -> Self {
        Self {
            dst,
            is_none,
            style,
        }
    }
}

impl<'d> serde::ser::Serializer for MapValueSerializer<'d> {
    type Ok = &'d mut String;
    type Error = Error;
    type SerializeSeq = SerializeValueArray<'d>;
    type SerializeTuple = SerializeValueArray<'d>;
    type SerializeTupleStruct = SerializeValueArray<'d>;
    type SerializeTupleVariant = SerializeTupleVariant<'d>;
    type SerializeMap = SerializeMap<'d>;
    type SerializeStruct = SerializeMap<'d>;
    type SerializeStructVariant = SerializeStructVariant<'d>;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_bool(v)
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_i8(v)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_i16(v)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_i32(v)
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_i64(v)
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_u8(v)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_u16(v)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_u32(v)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_u64(v)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_f32(v)
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_f64(v)
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_char(v)
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_str(v)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_bytes(value)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        *self.is_none = true;
        Err(Error::unsupported_none())
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        ValueSerializer::with_style(self.dst, self.style).serialize_some(value)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_unit()
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_unit_variant(
            name,
            variant_index,
            variant,
        )
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        ValueSerializer::with_style(self.dst, self.style).serialize_newtype_variant(
            name,
            variant_index,
            variant,
            value,
        )
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_seq(len)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_tuple(len)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_tuple_struct(name, len)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_tuple_variant(
            name,
            variant_index,
            variant,
            len,
        )
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_map(len)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_struct(name, len)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        ValueSerializer::with_style(self.dst, self.style).serialize_struct_variant(
            name,
            variant_index,
            variant,
            len,
        )
    }
}

pub struct SerializeStructVariant<'d> {
    inner: SerializeTable<'d>,
}

impl<'d> SerializeStructVariant<'d> {
    pub(crate) fn struct_(
        dst: &'d mut String,
        variant: &'static str,
        _len: usize,
        style: Style,
    ) -> Result<Self, Error> {
        dst.open_inline_table()?;
        dst.space()?;
        dst.key(variant)?;
        dst.space()?;
        dst.keyval_sep()?;
        dst.space()?;
        Ok(Self {
            inner: SerializeTable::map(dst, style)?,
        })
    }
}

impl<'d> serde::ser::SerializeStructVariant for SerializeStructVariant<'d> {
    type Ok = &'d mut String;
    type Error = Error;

    #[inline]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        serde::ser::SerializeStruct::serialize_field(&mut self.inner, key, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        let dst = serde::ser::SerializeStruct::end(self.inner)?;
        dst.space()?;
        dst.close_inline_table()?;
        Ok(dst)
    }
}
