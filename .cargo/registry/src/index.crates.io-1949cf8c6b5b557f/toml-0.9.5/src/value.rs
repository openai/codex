//! Definition of a TOML [value][Value]

use alloc::collections::BTreeMap;
use alloc::vec;
use core::fmt;
use core::hash::Hash;
use core::mem::discriminant;
use core::ops;
#[cfg(feature = "std")]
use std::collections::HashMap;

use serde::de;
use serde::de::IntoDeserializer;
use serde::ser;

use crate::alloc_prelude::*;

pub use toml_datetime::{Date, Datetime, DatetimeParseError, Offset, Time};

/// Type representing a TOML array, payload of the `Value::Array` variant
pub type Array = Vec<Value>;

#[doc(no_inline)]
pub use crate::Table;

/// Representation of a TOML value.
#[derive(PartialEq, Clone, Debug)]
pub enum Value {
    /// Represents a TOML string
    String(String),
    /// Represents a TOML integer
    Integer(i64),
    /// Represents a TOML float
    Float(f64),
    /// Represents a TOML boolean
    Boolean(bool),
    /// Represents a TOML datetime
    Datetime(Datetime),
    /// Represents a TOML array
    Array(Array),
    /// Represents a TOML table
    Table(Table),
}

impl Value {
    /// Convert a `T` into `toml::Value` which is an enum that can represent
    /// any valid TOML data.
    ///
    /// This conversion can fail if `T`'s implementation of `Serialize` decides to
    /// fail, or if `T` contains a map with non-string keys.
    pub fn try_from<T>(value: T) -> Result<Self, crate::ser::Error>
    where
        T: ser::Serialize,
    {
        value.serialize(ValueSerializer)
    }

    /// Interpret a `toml::Value` as an instance of type `T`.
    ///
    /// This conversion can fail if the structure of the `Value` does not match the
    /// structure expected by `T`, for example if `T` is a struct type but the
    /// `Value` contains something other than a TOML table. It can also fail if the
    /// structure is correct but `T`'s implementation of `Deserialize` decides that
    /// something is wrong with the data, for example required struct fields are
    /// missing from the TOML map or some number is too big to fit in the expected
    /// primitive type.
    pub fn try_into<'de, T>(self) -> Result<T, crate::de::Error>
    where
        T: de::Deserialize<'de>,
    {
        de::Deserialize::deserialize(self)
    }

    /// Index into a TOML array or map. A string index can be used to access a
    /// value in a map, and a usize index can be used to access an element of an
    /// array.
    ///
    /// Returns `None` if the type of `self` does not match the type of the
    /// index, for example if the index is a string and `self` is an array or a
    /// number. Also returns `None` if the given key does not exist in the map
    /// or the given index is not within the bounds of the array.
    pub fn get<I: Index>(&self, index: I) -> Option<&Self> {
        index.index(self)
    }

    /// Mutably index into a TOML array or map. A string index can be used to
    /// access a value in a map, and a usize index can be used to access an
    /// element of an array.
    ///
    /// Returns `None` if the type of `self` does not match the type of the
    /// index, for example if the index is a string and `self` is an array or a
    /// number. Also returns `None` if the given key does not exist in the map
    /// or the given index is not within the bounds of the array.
    pub fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Self> {
        index.index_mut(self)
    }

    /// Extracts the integer value if it is an integer.
    pub fn as_integer(&self) -> Option<i64> {
        match *self {
            Self::Integer(i) => Some(i),
            _ => None,
        }
    }

    /// Tests whether this value is an integer.
    pub fn is_integer(&self) -> bool {
        self.as_integer().is_some()
    }

    /// Extracts the float value if it is a float.
    pub fn as_float(&self) -> Option<f64> {
        match *self {
            Self::Float(f) => Some(f),
            _ => None,
        }
    }

    /// Tests whether this value is a float.
    pub fn is_float(&self) -> bool {
        self.as_float().is_some()
    }

    /// Extracts the boolean value if it is a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Self::Boolean(b) => Some(b),
            _ => None,
        }
    }

    /// Tests whether this value is a boolean.
    pub fn is_bool(&self) -> bool {
        self.as_bool().is_some()
    }

    /// Extracts the string of this value if it is a string.
    pub fn as_str(&self) -> Option<&str> {
        match *self {
            Self::String(ref s) => Some(&**s),
            _ => None,
        }
    }

    /// Tests if this value is a string.
    pub fn is_str(&self) -> bool {
        self.as_str().is_some()
    }

    /// Extracts the datetime value if it is a datetime.
    ///
    /// Note that a parsed TOML value will only contain ISO 8601 dates. An
    /// example date is:
    ///
    /// ```notrust
    /// 1979-05-27T07:32:00Z
    /// ```
    pub fn as_datetime(&self) -> Option<&Datetime> {
        match *self {
            Self::Datetime(ref s) => Some(s),
            _ => None,
        }
    }

    /// Tests whether this value is a datetime.
    pub fn is_datetime(&self) -> bool {
        self.as_datetime().is_some()
    }

    /// Extracts the array value if it is an array.
    pub fn as_array(&self) -> Option<&Vec<Self>> {
        match *self {
            Self::Array(ref s) => Some(s),
            _ => None,
        }
    }

    /// Extracts the array value if it is an array.
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Self>> {
        match *self {
            Self::Array(ref mut s) => Some(s),
            _ => None,
        }
    }

    /// Tests whether this value is an array.
    pub fn is_array(&self) -> bool {
        self.as_array().is_some()
    }

    /// Extracts the table value if it is a table.
    pub fn as_table(&self) -> Option<&Table> {
        match *self {
            Self::Table(ref s) => Some(s),
            _ => None,
        }
    }

    /// Extracts the table value if it is a table.
    pub fn as_table_mut(&mut self) -> Option<&mut Table> {
        match *self {
            Self::Table(ref mut s) => Some(s),
            _ => None,
        }
    }

    /// Tests whether this value is a table.
    pub fn is_table(&self) -> bool {
        self.as_table().is_some()
    }

    /// Tests whether this and another value have the same type.
    pub fn same_type(&self, other: &Self) -> bool {
        discriminant(self) == discriminant(other)
    }

    /// Returns a human-readable representation of the type of this value.
    pub fn type_str(&self) -> &'static str {
        match *self {
            Self::String(..) => "string",
            Self::Integer(..) => "integer",
            Self::Float(..) => "float",
            Self::Boolean(..) => "boolean",
            Self::Datetime(..) => "datetime",
            Self::Array(..) => "array",
            Self::Table(..) => "table",
        }
    }
}

impl<I> ops::Index<I> for Value
where
    I: Index,
{
    type Output = Self;

    fn index(&self, index: I) -> &Self {
        self.get(index).expect("index not found")
    }
}

impl<I> ops::IndexMut<I> for Value
where
    I: Index,
{
    fn index_mut(&mut self, index: I) -> &mut Self {
        self.get_mut(index).expect("index not found")
    }
}

impl<'a> From<&'a str> for Value {
    #[inline]
    fn from(val: &'a str) -> Self {
        Self::String(val.to_owned())
    }
}

impl<V: Into<Self>> From<Vec<V>> for Value {
    fn from(val: Vec<V>) -> Self {
        Self::Array(val.into_iter().map(|v| v.into()).collect())
    }
}

impl<S: Into<String>, V: Into<Self>> From<BTreeMap<S, V>> for Value {
    fn from(val: BTreeMap<S, V>) -> Self {
        let table = val.into_iter().map(|(s, v)| (s.into(), v.into())).collect();

        Self::Table(table)
    }
}

#[cfg(feature = "std")]
impl<S: Into<String> + Hash + Eq, V: Into<Self>> From<HashMap<S, V>> for Value {
    fn from(val: HashMap<S, V>) -> Self {
        let table = val.into_iter().map(|(s, v)| (s.into(), v.into())).collect();

        Self::Table(table)
    }
}

macro_rules! impl_into_value {
    ($variant:ident : $T:ty) => {
        impl From<$T> for Value {
            #[inline]
            fn from(val: $T) -> Value {
                Value::$variant(val.into())
            }
        }
    };
}

impl_into_value!(String: String);
impl_into_value!(Integer: i64);
impl_into_value!(Integer: i32);
impl_into_value!(Integer: i8);
impl_into_value!(Integer: u8);
impl_into_value!(Integer: u32);
impl_into_value!(Float: f64);
impl_into_value!(Float: f32);
impl_into_value!(Boolean: bool);
impl_into_value!(Datetime: Datetime);
impl_into_value!(Table: Table);

/// Types that can be used to index a `toml::Value`
///
/// Currently this is implemented for `usize` to index arrays and `str` to index
/// tables.
///
/// This trait is sealed and not intended for implementation outside of the
/// `toml` crate.
pub trait Index: Sealed {
    #[doc(hidden)]
    fn index<'a>(&self, val: &'a Value) -> Option<&'a Value>;
    #[doc(hidden)]
    fn index_mut<'a>(&self, val: &'a mut Value) -> Option<&'a mut Value>;
}

/// An implementation detail that should not be implemented, this will change in
/// the future and break code otherwise.
#[doc(hidden)]
pub trait Sealed {}
impl Sealed for usize {}
impl Sealed for str {}
impl Sealed for String {}
impl<T: Sealed + ?Sized> Sealed for &T {}

impl Index for usize {
    fn index<'a>(&self, val: &'a Value) -> Option<&'a Value> {
        match *val {
            Value::Array(ref a) => a.get(*self),
            _ => None,
        }
    }

    fn index_mut<'a>(&self, val: &'a mut Value) -> Option<&'a mut Value> {
        match *val {
            Value::Array(ref mut a) => a.get_mut(*self),
            _ => None,
        }
    }
}

impl Index for str {
    fn index<'a>(&self, val: &'a Value) -> Option<&'a Value> {
        match *val {
            Value::Table(ref a) => a.get(self),
            _ => None,
        }
    }

    fn index_mut<'a>(&self, val: &'a mut Value) -> Option<&'a mut Value> {
        match *val {
            Value::Table(ref mut a) => a.get_mut(self),
            _ => None,
        }
    }
}

impl Index for String {
    fn index<'a>(&self, val: &'a Value) -> Option<&'a Value> {
        self[..].index(val)
    }

    fn index_mut<'a>(&self, val: &'a mut Value) -> Option<&'a mut Value> {
        self[..].index_mut(val)
    }
}

impl<T> Index for &T
where
    T: Index + ?Sized,
{
    fn index<'a>(&self, val: &'a Value) -> Option<&'a Value> {
        (**self).index(val)
    }

    fn index_mut<'a>(&self, val: &'a mut Value) -> Option<&'a mut Value> {
        (**self).index_mut(val)
    }
}

#[cfg(feature = "display")]
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use serde::Serialize as _;

        let mut output = String::new();
        let serializer = crate::ser::ValueSerializer::new(&mut output);
        self.serialize(serializer).unwrap();
        output.fmt(f)
    }
}

#[cfg(feature = "parse")]
impl core::str::FromStr for Value {
    type Err = crate::de::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use serde::Deserialize as _;
        Self::deserialize(crate::de::ValueDeserializer::parse(s)?)
    }
}

impl ser::Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *self {
            Self::String(ref s) => serializer.serialize_str(s),
            Self::Integer(i) => serializer.serialize_i64(i),
            Self::Float(f) => serializer.serialize_f64(f),
            Self::Boolean(b) => serializer.serialize_bool(b),
            Self::Datetime(ref s) => s.serialize(serializer),
            Self::Array(ref a) => a.serialize(serializer),
            Self::Table(ref t) => t.serialize(serializer),
        }
    }
}

impl<'de> de::Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct ValueVisitor;

        impl<'de> de::Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("any valid TOML value")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
                Ok(Value::Boolean(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
                Ok(Value::Integer(value))
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
                if i64::try_from(value).is_ok() {
                    Ok(Value::Integer(value as i64))
                } else {
                    Err(de::Error::custom("u64 value was too large"))
                }
            }

            fn visit_u32<E>(self, value: u32) -> Result<Value, E> {
                Ok(Value::Integer(value.into()))
            }

            fn visit_i32<E>(self, value: i32) -> Result<Value, E> {
                Ok(Value::Integer(value.into()))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
                Ok(Value::Float(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<Value, E> {
                Ok(Value::String(value.into()))
            }

            fn visit_string<E>(self, value: String) -> Result<Value, E> {
                Ok(Value::String(value))
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Value, D::Error>
            where
                D: de::Deserializer<'de>,
            {
                de::Deserialize::deserialize(deserializer)
            }

            fn visit_seq<V>(self, mut visitor: V) -> Result<Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let mut vec = Vec::new();
                while let Some(elem) = visitor.next_element()? {
                    vec.push(elem);
                }
                Ok(Value::Array(vec))
            }

            fn visit_map<V>(self, mut visitor: V) -> Result<Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let key = match toml_datetime::de::VisitMap::next_key_seed(&mut visitor)? {
                    Some(toml_datetime::de::VisitMap::Datetime(datetime)) => {
                        return Ok(Value::Datetime(datetime));
                    }
                    None => return Ok(Value::Table(Table::new())),
                    Some(toml_datetime::de::VisitMap::Key(key)) => key,
                };
                let mut map = Table::new();
                map.insert(key.into_owned(), visitor.next_value()?);
                while let Some(key) = visitor.next_key::<String>()? {
                    if let crate::map::Entry::Vacant(vacant) = map.entry(&key) {
                        vacant.insert(visitor.next_value()?);
                    } else {
                        let msg = format!("duplicate key: `{key}`");
                        return Err(de::Error::custom(msg));
                    }
                }
                Ok(Value::Table(map))
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

// This is wrapped by `Table` and any trait methods implemented here need to be wrapped there.
impl<'de> de::Deserializer<'de> for Value {
    type Error = crate::de::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, crate::de::Error>
    where
        V: de::Visitor<'de>,
    {
        match self {
            Self::Boolean(v) => visitor.visit_bool(v),
            Self::Integer(n) => visitor.visit_i64(n),
            Self::Float(n) => visitor.visit_f64(n),
            Self::String(v) => visitor.visit_string(v),
            Self::Datetime(v) => visitor.visit_string(v.to_string()),
            Self::Array(v) => {
                let len = v.len();
                let mut deserializer = SeqDeserializer::new(v);
                let seq = visitor.visit_seq(&mut deserializer)?;
                let remaining = deserializer.iter.len();
                if remaining == 0 {
                    Ok(seq)
                } else {
                    Err(de::Error::invalid_length(len, &"fewer elements in array"))
                }
            }
            Self::Table(v) => {
                let len = v.len();
                let mut deserializer = MapDeserializer::new(v);
                let map = visitor.visit_map(&mut deserializer)?;
                let remaining = deserializer.iter.len();
                if remaining == 0 {
                    Ok(map)
                } else {
                    Err(de::Error::invalid_length(len, &"fewer elements in map"))
                }
            }
        }
    }

    #[inline]
    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, crate::de::Error>
    where
        V: de::Visitor<'de>,
    {
        match self {
            Self::String(variant) => visitor.visit_enum(variant.into_deserializer()),
            Self::Table(variant) => {
                if variant.is_empty() {
                    Err(crate::de::Error::custom(
                        "wanted exactly 1 element, found 0 elements",
                        None,
                    ))
                } else if variant.len() != 1 {
                    Err(crate::de::Error::custom(
                        "wanted exactly 1 element, more than 1 element",
                        None,
                    ))
                } else {
                    let deserializer = MapDeserializer::new(variant);
                    visitor.visit_enum(deserializer)
                }
            }
            _ => Err(de::Error::invalid_type(
                de::Unexpected::UnitVariant,
                &"string only",
            )),
        }
    }

    // `None` is interpreted as a missing field so be sure to implement `Some`
    // as a present field.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, crate::de::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, crate::de::Error>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string unit seq
        bytes byte_buf map unit_struct tuple_struct struct
        tuple ignored_any identifier
    }
}

pub(crate) struct SeqDeserializer {
    iter: vec::IntoIter<Value>,
}

impl SeqDeserializer {
    fn new(vec: Vec<Value>) -> Self {
        Self {
            iter: vec.into_iter(),
        }
    }
}

impl<'de> de::SeqAccess<'de> for SeqDeserializer {
    type Error = crate::de::Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, crate::de::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(value) => seed.deserialize(value).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        match self.iter.size_hint() {
            (lower, Some(upper)) if lower == upper => Some(upper),
            _ => None,
        }
    }
}

pub(crate) struct MapDeserializer {
    iter: <Table as IntoIterator>::IntoIter,
    value: Option<(String, Value)>,
}

impl MapDeserializer {
    fn new(map: Table) -> Self {
        Self {
            iter: map.into_iter(),
            value: None,
        }
    }
}

impl<'de> de::MapAccess<'de> for MapDeserializer {
    type Error = crate::de::Error;

    fn next_key_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, crate::de::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some((key.clone(), value));
                seed.deserialize(Value::String(key)).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<T>(&mut self, seed: T) -> Result<T::Value, crate::de::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        let (key, res) = match self.value.take() {
            Some((key, value)) => (key, seed.deserialize(value)),
            None => return Err(de::Error::custom("value is missing")),
        };
        res.map_err(|mut error| {
            error.add_key(key);
            error
        })
    }

    fn size_hint(&self) -> Option<usize> {
        match self.iter.size_hint() {
            (lower, Some(upper)) if lower == upper => Some(upper),
            _ => None,
        }
    }
}

impl<'de> de::EnumAccess<'de> for MapDeserializer {
    type Error = crate::de::Error;
    type Variant = MapEnumDeserializer;

    fn variant_seed<V>(mut self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        use de::Error;
        let (key, value) = match self.iter.next() {
            Some(pair) => pair,
            None => {
                return Err(Error::custom(
                    "expected table with exactly 1 entry, found empty table",
                ));
            }
        };

        let val = seed.deserialize(key.into_deserializer())?;

        let variant = MapEnumDeserializer::new(value);

        Ok((val, variant))
    }
}

/// Deserializes table values into enum variants.
pub(crate) struct MapEnumDeserializer {
    value: Value,
}

impl MapEnumDeserializer {
    pub(crate) fn new(value: Value) -> Self {
        Self { value }
    }
}

impl<'de> de::VariantAccess<'de> for MapEnumDeserializer {
    type Error = crate::de::Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        use de::Error;
        match self.value {
            Value::Array(values) => {
                if values.is_empty() {
                    Ok(())
                } else {
                    Err(Error::custom("expected empty array"))
                }
            }
            Value::Table(values) => {
                if values.is_empty() {
                    Ok(())
                } else {
                    Err(Error::custom("expected empty table"))
                }
            }
            e => Err(Error::custom(format!(
                "expected table, found {}",
                e.type_str()
            ))),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self.value.into_deserializer())
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        use de::Error;
        match self.value {
            Value::Array(values) => {
                if values.len() == len {
                    de::Deserializer::deserialize_seq(values.into_deserializer(), visitor)
                } else {
                    Err(Error::custom(format!("expected tuple with length {len}")))
                }
            }
            Value::Table(values) => {
                let tuple_values: Result<Vec<_>, _> = values
                    .into_iter()
                    .enumerate()
                    .map(|(index, (key, value))| match key.parse::<usize>() {
                        Ok(key_index) if key_index == index => Ok(value),
                        Ok(_) | Err(_) => Err(Error::custom(format!(
                            "expected table key `{index}`, but was `{key}`"
                        ))),
                    })
                    .collect();
                let tuple_values = tuple_values?;

                if tuple_values.len() == len {
                    de::Deserializer::deserialize_seq(tuple_values.into_deserializer(), visitor)
                } else {
                    Err(Error::custom(format!("expected tuple with length {len}")))
                }
            }
            e => Err(Error::custom(format!(
                "expected table, found {}",
                e.type_str()
            ))),
        }
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        de::Deserializer::deserialize_struct(
            self.value.into_deserializer(),
            "", // TODO: this should be the variant name
            fields,
            visitor,
        )
    }
}

impl IntoDeserializer<'_, crate::de::Error> for Value {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

pub(crate) struct ValueSerializer;

impl ser::Serializer for ValueSerializer {
    type Ok = Value;
    type Error = crate::ser::Error;

    type SerializeSeq = ValueSerializeVec;
    type SerializeTuple = ValueSerializeVec;
    type SerializeTupleStruct = ValueSerializeVec;
    type SerializeTupleVariant = ValueSerializeTupleVariant;
    type SerializeMap = ValueSerializeMap;
    type SerializeStruct = ValueSerializeMap;
    type SerializeStructVariant = ValueSerializeStructVariant;

    fn serialize_bool(self, value: bool) -> Result<Value, crate::ser::Error> {
        Ok(Value::Boolean(value))
    }

    fn serialize_i8(self, value: i8) -> Result<Value, crate::ser::Error> {
        self.serialize_i64(value.into())
    }

    fn serialize_i16(self, value: i16) -> Result<Value, crate::ser::Error> {
        self.serialize_i64(value.into())
    }

    fn serialize_i32(self, value: i32) -> Result<Value, crate::ser::Error> {
        self.serialize_i64(value.into())
    }

    fn serialize_i64(self, value: i64) -> Result<Value, crate::ser::Error> {
        Ok(Value::Integer(value))
    }

    fn serialize_u8(self, value: u8) -> Result<Value, crate::ser::Error> {
        self.serialize_i64(value.into())
    }

    fn serialize_u16(self, value: u16) -> Result<Value, crate::ser::Error> {
        self.serialize_i64(value.into())
    }

    fn serialize_u32(self, value: u32) -> Result<Value, crate::ser::Error> {
        self.serialize_i64(value.into())
    }

    fn serialize_u64(self, value: u64) -> Result<Value, crate::ser::Error> {
        if i64::try_from(value).is_ok() {
            self.serialize_i64(value as i64)
        } else {
            Err(ser::Error::custom("u64 value was too large"))
        }
    }

    fn serialize_f32(self, value: f32) -> Result<Value, crate::ser::Error> {
        self.serialize_f64(value as f64)
    }

    fn serialize_f64(self, mut value: f64) -> Result<Value, crate::ser::Error> {
        // Discard sign of NaN. See ValueSerializer::serialize_f64.
        if value.is_nan() {
            value = value.copysign(1.0);
        }
        Ok(Value::Float(value))
    }

    fn serialize_char(self, value: char) -> Result<Value, crate::ser::Error> {
        let mut s = String::new();
        s.push(value);
        self.serialize_str(&s)
    }

    fn serialize_str(self, value: &str) -> Result<Value, crate::ser::Error> {
        Ok(Value::String(value.to_owned()))
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Value, crate::ser::Error> {
        let vec = value.iter().map(|&b| Value::Integer(b.into())).collect();
        Ok(Value::Array(vec))
    }

    fn serialize_unit(self) -> Result<Value, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(Some("unit")))
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Value, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_type(Some(name)))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Value, crate::ser::Error> {
        self.serialize_str(_variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value, crate::ser::Error>
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
    ) -> Result<Value, crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        let value = value.serialize(Self)?;
        let mut table = Table::new();
        table.insert(variant.to_owned(), value);
        Ok(table.into())
    }

    fn serialize_none(self) -> Result<Value, crate::ser::Error> {
        Err(crate::ser::Error::unsupported_none())
    }

    fn serialize_some<T>(self, value: &T) -> Result<Value, crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        value.serialize(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, crate::ser::Error> {
        Ok(ValueSerializeVec {
            vec: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, crate::ser::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, crate::ser::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, crate::ser::Error> {
        Ok(ValueSerializeTupleVariant::tuple(variant, len))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, crate::ser::Error> {
        Ok(ValueSerializeMap {
            ser: crate::table::SerializeMap::new(),
        })
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
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, crate::ser::Error> {
        Ok(ValueSerializeStructVariant::struct_(variant, len))
    }
}

pub(crate) struct ValueSerializeVec {
    vec: Vec<Value>,
}

impl ser::SerializeSeq for ValueSerializeVec {
    type Ok = Value;
    type Error = crate::ser::Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        self.vec.push(Value::try_from(value)?);
        Ok(())
    }

    fn end(self) -> Result<Value, crate::ser::Error> {
        Ok(Value::Array(self.vec))
    }
}

impl ser::SerializeTuple for ValueSerializeVec {
    type Ok = Value;
    type Error = crate::ser::Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, crate::ser::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for ValueSerializeVec {
    type Ok = Value;
    type Error = crate::ser::Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, crate::ser::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleVariant for ValueSerializeVec {
    type Ok = Value;
    type Error = crate::ser::Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, crate::ser::Error> {
        ser::SerializeSeq::end(self)
    }
}

pub(crate) struct ValueSerializeMap {
    ser: crate::table::SerializeMap,
}

impl ser::SerializeMap for ValueSerializeMap {
    type Ok = Value;
    type Error = crate::ser::Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        self.ser.serialize_key(key)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        self.ser.serialize_value(value)
    }

    fn end(self) -> Result<Value, crate::ser::Error> {
        self.ser.end().map(Value::Table)
    }
}

impl ser::SerializeStruct for ValueSerializeMap {
    type Ok = Value;
    type Error = crate::ser::Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), crate::ser::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        ser::SerializeMap::serialize_key(self, key)?;
        ser::SerializeMap::serialize_value(self, value)
    }

    fn end(self) -> Result<Value, crate::ser::Error> {
        ser::SerializeMap::end(self)
    }
}

type ValueSerializeTupleVariant = ValueSerializeVariant<ValueSerializeVec>;
type ValueSerializeStructVariant = ValueSerializeVariant<ValueSerializeMap>;

pub(crate) struct ValueSerializeVariant<T> {
    variant: &'static str,
    inner: T,
}

impl ValueSerializeVariant<ValueSerializeVec> {
    pub(crate) fn tuple(variant: &'static str, len: usize) -> Self {
        Self {
            variant,
            inner: ValueSerializeVec {
                vec: Vec::with_capacity(len),
            },
        }
    }
}

impl ValueSerializeVariant<ValueSerializeMap> {
    pub(crate) fn struct_(variant: &'static str, len: usize) -> Self {
        Self {
            variant,
            inner: ValueSerializeMap {
                ser: crate::table::SerializeMap::with_capacity(len),
            },
        }
    }
}

impl ser::SerializeTupleVariant for ValueSerializeVariant<ValueSerializeVec> {
    type Ok = Value;
    type Error = crate::ser::Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        ser::SerializeSeq::serialize_element(&mut self.inner, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let inner = ser::SerializeSeq::end(self.inner)?;
        let mut table = Table::new();
        table.insert(self.variant.to_owned(), inner);
        Ok(Value::Table(table))
    }
}

impl ser::SerializeStructVariant for ValueSerializeVariant<ValueSerializeMap> {
    type Ok = Value;
    type Error = crate::ser::Error;

    #[inline]
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ser::Serialize + ?Sized,
    {
        ser::SerializeStruct::serialize_field(&mut self.inner, key, value)
    }

    #[inline]
    fn end(self) -> Result<Self::Ok, Self::Error> {
        let inner = ser::SerializeStruct::end(self.inner)?;
        let mut table = Table::new();
        table.insert(self.variant.to_owned(), inner);
        Ok(Value::Table(table))
    }
}
