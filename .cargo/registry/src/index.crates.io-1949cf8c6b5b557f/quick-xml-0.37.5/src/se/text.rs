//! Contains serializer for a special `&text` field

use crate::de::TEXT_KEY;
use crate::se::simple_type::{SimpleSeq, SimpleTypeSerializer};
use crate::se::SeError;
use serde::ser::{Impossible, Serialize, Serializer};
use serde::serde_if_integer128;
use std::fmt::Write;

macro_rules! write_primitive {
    ($method:ident ( $ty:ty )) => {
        #[inline]
        fn $method(self, value: $ty) -> Result<Self::Ok, Self::Error> {
            self.0.$method(value)
        }
    };
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/// A serializer used to serialize a `$text` field of a struct or map.
///
/// This serializer a very similar to [`SimpleTypeSerializer`], but different
/// from it in how it processes unit enum variants. Unlike [`SimpleTypeSerializer`]
/// this serializer does not write anything for the unit variant.
pub struct TextSerializer<W: Write>(pub SimpleTypeSerializer<W>);

impl<W: Write> Serializer for TextSerializer<W> {
    type Ok = W;
    type Error = SeError;

    type SerializeSeq = SimpleSeq<W>;
    type SerializeTuple = SimpleSeq<W>;
    type SerializeTupleStruct = SimpleSeq<W>;
    type SerializeTupleVariant = SimpleSeq<W>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    write_primitive!(serialize_bool(bool));

    write_primitive!(serialize_i8(i8));
    write_primitive!(serialize_i16(i16));
    write_primitive!(serialize_i32(i32));
    write_primitive!(serialize_i64(i64));

    write_primitive!(serialize_u8(u8));
    write_primitive!(serialize_u16(u16));
    write_primitive!(serialize_u32(u32));
    write_primitive!(serialize_u64(u64));

    serde_if_integer128! {
        write_primitive!(serialize_i128(i128));
        write_primitive!(serialize_u128(u128));
    }

    write_primitive!(serialize_f32(f32));
    write_primitive!(serialize_f64(f64));

    write_primitive!(serialize_char(char));
    write_primitive!(serialize_str(&str));
    write_primitive!(serialize_bytes(&[u8]));

    #[inline]
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_none()
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    #[inline]
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit()
    }

    #[inline]
    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_struct(name)
    }

    #[inline]
    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        if variant == TEXT_KEY {
            Ok(self.0.writer)
        } else {
            self.0.serialize_unit_variant(name, variant_index, variant)
        }
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    #[inline]
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(SeError::Unsupported(
            format!(
                "cannot serialize enum newtype variant `{}::{}` as text content value",
                name, variant
            )
            .into(),
        ))
    }

    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.0.serialize_seq(len)
    }

    #[inline]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.0.serialize_tuple(len)
    }

    #[inline]
    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.0.serialize_tuple_struct(name, len)
    }

    #[inline]
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(SeError::Unsupported(
            format!(
                "cannot serialize enum tuple variant `{}::{}` as text content value",
                name, variant
            )
            .into(),
        ))
    }

    #[inline]
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(SeError::Unsupported(
            "cannot serialize map as text content value".into(),
        ))
    }

    #[inline]
    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(SeError::Unsupported(
            format!("cannot serialize struct `{}` as text content value", name).into(),
        ))
    }

    #[inline]
    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(SeError::Unsupported(
            format!(
                "cannot serialize enum struct variant `{}::{}` as text content value",
                name, variant
            )
            .into(),
        ))
    }
}
