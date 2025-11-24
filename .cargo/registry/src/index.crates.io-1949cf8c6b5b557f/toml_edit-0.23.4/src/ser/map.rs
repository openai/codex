use super::array::SerializeTupleVariant;
use super::array::SerializeValueArray;
use super::key::KeySerializer;
use super::value::ValueSerializer;
use super::Error;

#[doc(hidden)]
#[allow(clippy::large_enum_variant)]
pub enum SerializeMap {
    Datetime(SerializeDatetime),
    Table(SerializeInlineTable),
}

impl SerializeMap {
    pub(crate) fn map(len: Option<usize>) -> Self {
        Self::Table(SerializeInlineTable::map(len))
    }

    pub(crate) fn struct_(name: &'static str, len: Option<usize>) -> Self {
        if toml_datetime::ser::is_datetime(name) {
            Self::Datetime(SerializeDatetime::new())
        } else {
            Self::map(len)
        }
    }
}

impl serde::ser::SerializeMap for SerializeMap {
    type Ok = crate::Value;
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
            Self::Datetime(s) => s.end().map(|items| items.into()),
            Self::Table(s) => s.end().map(|items| items.into()),
        }
    }
}

impl serde::ser::SerializeStruct for SerializeMap {
    type Ok = crate::Value;
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
            Self::Datetime(s) => s.end().map(|items| items.into()),
            Self::Table(s) => s.end().map(|items| items.into()),
        }
    }
}

#[doc(hidden)]
pub struct SerializeDatetime {
    inner: toml_datetime::ser::DatetimeSerializer,
}

impl SerializeDatetime {
    pub(crate) fn new() -> Self {
        Self {
            inner: toml_datetime::ser::DatetimeSerializer::new(),
        }
    }
}

impl serde::ser::SerializeMap for SerializeDatetime {
    type Ok = crate::Datetime;
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

impl serde::ser::SerializeStruct for SerializeDatetime {
    type Ok = crate::Datetime;
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
        Ok(value)
    }
}

fn dt_err(err: toml_datetime::ser::SerializerError) -> Error {
    match err {
        toml_datetime::ser::SerializerError::InvalidFormat(err) => Error::custom(err),
        _ => Error::date_invalid(),
    }
}

#[doc(hidden)]
pub struct SerializeInlineTable {
    items: crate::table::KeyValuePairs,
    key: Option<crate::Key>,
}

impl SerializeInlineTable {
    pub(crate) fn map(len: Option<usize>) -> Self {
        let mut items: crate::table::KeyValuePairs = Default::default();
        let key = Default::default();
        if let Some(len) = len {
            items.reserve(len);
        }
        Self { items, key }
    }
}

impl serde::ser::SerializeMap for SerializeInlineTable {
    type Ok = crate::InlineTable;
    type Error = Error;

    fn serialize_key<T>(&mut self, input: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        self.key = Some(input.serialize(KeySerializer)?);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let mut is_none = false;
        let value_serializer = MapValueSerializer::new(&mut is_none);
        let res = value.serialize(value_serializer);
        match res {
            Ok(item) => {
                let key = self.key.take().unwrap();
                let item = crate::Item::Value(item);
                self.items.insert(key, item);
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
        Ok(crate::InlineTable::with_pairs(self.items))
    }
}

impl serde::ser::SerializeStruct for SerializeInlineTable {
    type Ok = crate::InlineTable;
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        let mut is_none = false;
        let value_serializer = MapValueSerializer::new(&mut is_none);
        let res = value.serialize(value_serializer);
        match res {
            Ok(item) => {
                let item = crate::Item::Value(item);
                self.items.insert(crate::Key::new(key), item);
            }
            Err(e) => {
                if !(e == Error::unsupported_none() && is_none) {
                    return Err(e);
                }
            }
        };
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(crate::InlineTable::with_pairs(self.items))
    }
}

struct MapValueSerializer<'d> {
    is_none: &'d mut bool,
}

impl<'d> MapValueSerializer<'d> {
    fn new(is_none: &'d mut bool) -> Self {
        Self { is_none }
    }
}

impl serde::ser::Serializer for MapValueSerializer<'_> {
    type Ok = crate::Value;
    type Error = Error;
    type SerializeSeq = SerializeValueArray;
    type SerializeTuple = SerializeValueArray;
    type SerializeTupleStruct = SerializeValueArray;
    type SerializeTupleVariant = SerializeTupleVariant;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeMap;
    type SerializeStructVariant = SerializeStructVariant;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_bool(v)
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_i8(v)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_i16(v)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_i32(v)
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_i64(v)
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_u8(v)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_u16(v)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_u32(v)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_u64(v)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_f32(v)
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_f64(v)
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_char(v)
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_str(v)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_bytes(value)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        *self.is_none = true;
        Err(Error::unsupported_none())
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::ser::Serialize + ?Sized,
    {
        ValueSerializer::new().serialize_some(value)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_unit()
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_unit_struct(name)
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        ValueSerializer::new().serialize_unit_variant(name, variant_index, variant)
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
        ValueSerializer::new().serialize_newtype_variant(name, variant_index, variant, value)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        ValueSerializer::new().serialize_seq(len)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        ValueSerializer::new().serialize_tuple(len)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        ValueSerializer::new().serialize_tuple_struct(name, len)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        ValueSerializer::new().serialize_tuple_variant(name, variant_index, variant, len)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        ValueSerializer::new().serialize_map(len)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        ValueSerializer::new().serialize_struct(name, len)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        ValueSerializer::new().serialize_struct_variant(name, variant_index, variant, len)
    }
}

pub struct SerializeStructVariant {
    variant: &'static str,
    inner: SerializeInlineTable,
}

impl SerializeStructVariant {
    pub(crate) fn struct_(variant: &'static str, len: usize) -> Self {
        Self {
            variant,
            inner: SerializeInlineTable::map(Some(len)),
        }
    }
}

impl serde::ser::SerializeStructVariant for SerializeStructVariant {
    type Ok = crate::Value;
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
        let inner = serde::ser::SerializeStruct::end(self.inner)?.into();
        let mut items = crate::table::KeyValuePairs::new();
        let value = crate::Item::Value(inner);
        items.insert(crate::Key::new(self.variant), value);
        Ok(crate::Value::InlineTable(crate::InlineTable::with_pairs(
            items,
        )))
    }
}
