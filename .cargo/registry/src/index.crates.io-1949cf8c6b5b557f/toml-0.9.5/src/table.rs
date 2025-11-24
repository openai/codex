use serde::de;
use serde::ser;

use crate::alloc_prelude::*;
use crate::map::Map;
use crate::Value;

/// Type representing a TOML table, payload of the `Value::Table` variant.
///
/// By default it entries are stored in
/// [lexicographic order](https://doc.rust-lang.org/std/primitive.str.html#impl-Ord-for-str)
/// of the keys. Enable the `preserve_order` feature to store entries in the order they appear in
/// the source file.
pub type Table = Map<String, Value>;

impl Table {
    /// Convert a `T` into `toml::Table`.
    ///
    /// This conversion can fail if `T`'s implementation of `Serialize` decides to
    /// fail, or if `T` contains a map with non-string keys.
    pub fn try_from<T>(value: T) -> Result<Self, crate::ser::Error>
    where
        T: ser::Serialize,
    {
        value.serialize(TableSerializer)
    }

    /// Interpret a `toml::Table` as an instance of type `T`.
    ///
    /// This conversion can fail if the structure of the `Table` does not match the structure
    /// expected by `T`, for example if `T` is a bool which can't be mapped to a `Table`. It can
    /// also fail if the structure is correct but `T`'s implementation of `Deserialize` decides
    /// that something is wrong with the data, for example required struct fields are missing from
    /// the TOML map or some number is too big to fit in the expected primitive type.
    pub fn try_into<'de, T>(self) -> Result<T, crate::de::Error>
    where
        T: de::Deserialize<'de>,
    {
        de::Deserialize::deserialize(self)
    }
}

#[cfg(feature = "display")]
impl core::fmt::Display for Table {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::ser::to_string(self)
            .expect("Unable to represent value as string")
            .fmt(f)
    }
}

#[cfg(feature = "parse")]
impl core::str::FromStr for Table {
    type Err = crate::de::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::from_str(s)
    }
}
impl ser::Serialize for Table {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (k, v) in self {
            map.serialize_key(k)?;
            map.serialize_value(v)?;
        }
        map.end()
    }
}

impl<'de> de::Deserialize<'de> for Table {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Map<String, Value>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("a map")
            }

            #[inline]
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(Map::new())
            }

            #[inline]
            fn visit_map<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut values = Map::new();

                while let Some((key, value)) = visitor.next_entry()? {
                    values.insert(key, value);
                }

                Ok(values)
            }
        }

        deserializer.deserialize_map(Visitor)
    }
}

impl<'de> de::Deserializer<'de> for Table {
    type Error = crate::de::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, crate::de::Error>
    where
        V: de::Visitor<'de>,
    {
        Value::Table(self).deserialize_any(visitor)
    }

    #[inline]
    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, crate::de::Error>
    where
        V: de::Visitor<'de>,
    {
        Value::Table(self).deserialize_enum(name, variants, visitor)
    }

    // `None` is interpreted as a missing field so be sure to implement `Some`
    // as a present field.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, crate::de::Error>
    where
        V: de::Visitor<'de>,
    {
        Value::Table(self).deserialize_option(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, crate::de::Error>
    where
        V: de::Visitor<'de>,
    {
        Value::Table(self).deserialize_newtype_struct(name, visitor)
    }

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string unit seq
        bytes byte_buf map unit_struct tuple_struct struct
        tuple ignored_any identifier
    }
}

impl de::IntoDeserializer<'_, crate::de::Error> for Table {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

pub(crate) struct TableSerializer;

impl ser::Serializer for TableSerializer {
    type Ok = Table;
    type Error = crate::ser::Error;

    type SerializeSeq = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = SerializeMap;
    type SerializeStruct = SerializeMap;
    type SerializeStructVariant = ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, _value: bool) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_i8(self, _value: i8) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_i16(self, _value: i16) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_i32(self, _value: i32) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_i64(self, _value: i64) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_u8(self, _value: u8) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_u16(self, _value: u16) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_u32(self, _value: u32) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_u64(self, _value: u64) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_f32(self, _value: f32) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_f64(self, _value: f64) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_char(self, _value: char) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_str(self, _value: &str) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_unit(self) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(Some(name)))
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Table, crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Table, crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        let value = value.serialize(crate::value::ValueSerializer)?;
        let mut table = Table::new();
        table.insert(variant.to_owned(), value);
        Ok(table)
    }

    fn serialize_none(self) -> Result<Table, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_none())
    }

    fn serialize_some<T>(self, value: &T) -> Result<Table, crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(None))
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(Some(name)))
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(Some(name)))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, crate::ser::Error> {
        Ok(SerializeMap::new())
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, crate::ser::Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(Some(name)))
    }
}

pub(crate) struct SerializeMap {
    map: Table,
    next_key: Option<String>,
}

impl SerializeMap {
    pub(crate) fn new() -> Self {
        Self {
            map: Table::new(),
            next_key: None,
        }
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            map: Table::with_capacity(capacity),
            next_key: None,
        }
    }
}

impl ser::SerializeMap for SerializeMap {
    type Ok = Table;
    type Error = crate::ser::Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        match Value::try_from(key)? {
            Value::String(s) => self.next_key = Some(s),
            _ => return Err(crate::ser::Error::key_not_string()),
        };
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        let key = self.next_key.take();
        let key = key.expect("serialize_value called before serialize_key");
        match Value::try_from(value) {
            Ok(value) => {
                self.map.insert(key, value);
            }
            Err(crate::ser::Error {
                inner: crate::ser::ErrorInner::UnsupportedNone,
            }) => {}
            Err(e) => return Err(e),
        }
        Ok(())
    }

    fn end(self) -> Result<Table, crate::ser::Error> {
        Ok(self.map)
    }
}

impl ser::SerializeStruct for SerializeMap {
    type Ok = Table;
    type Error = crate::ser::Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        ser::SerializeMap::serialize_key(self, key)?;
        ser::SerializeMap::serialize_value(self, value)
    }

    fn end(self) -> Result<Table, crate::ser::Error> {
        ser::SerializeMap::end(self)
    }
}
