use crate::{
    content::ser::{Content, ContentSerializer},
    prelude::*,
};

/// Represent a list of enum values as a map.
///
/// This **only works** if the enum uses the default *externally tagged* representation.
/// Other enum representations are not supported.
///
/// serde data formats often represent *externally tagged* enums as maps with a single key.
/// The key is the enum variant name, and the value is the variant value.
/// Sometimes a map with multiple keys should be treated like a list of enum values.
///
/// # Examples
///
/// ## JSON Map with multiple keys
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// use serde_with::{serde_as, EnumMap};
///
/// # #[derive(Debug, Clone, PartialEq, Eq)]
/// #[derive(Serialize, Deserialize)]
/// enum EnumValue {
///     Int(i32),
///     String(String),
///     Unit,
///     Tuple(i32, String, bool),
///     Struct {
///         a: i32,
///         b: String,
///         c: bool,
///     },
/// }
///
/// #[serde_as]
/// # #[derive(Debug, Clone, PartialEq, Eq)]
/// #[derive(Serialize, Deserialize)]
/// struct VecEnumValues (
///     #[serde_as(as = "EnumMap")]
///     Vec<EnumValue>,
/// );
///
/// // ---
///
/// // This will serialize this list of values
/// let values = VecEnumValues(vec![
///     EnumValue::Int(123),
///     EnumValue::String("FooBar".to_string()),
///     EnumValue::Int(456),
///     EnumValue::String("XXX".to_string()),
///     EnumValue::Unit,
///     EnumValue::Tuple(1, "Middle".to_string(), false),
///     EnumValue::Struct {
///         a: 666,
///         b: "BBB".to_string(),
///         c: true,
///     },
/// ]);
///
/// // into this JSON map
/// // Duplicate keys are emitted for identical enum variants.
/// let expected =
/// r#"{
///   "Int": 123,
///   "String": "FooBar",
///   "Int": 456,
///   "String": "XXX",
///   "Unit": null,
///   "Tuple": [
///     1,
///     "Middle",
///     false
///   ],
///   "Struct": {
///     "a": 666,
///     "b": "BBB",
///     "c": true
///   }
/// }"#;
///
/// // Both serialization and deserialization work flawlessly.
/// let serialized = serde_json::to_string_pretty(&values).unwrap();
/// assert_eq!(expected, serialized);
/// let deserialized: VecEnumValues = serde_json::from_str(&serialized).unwrap();
/// assert_eq!(values, deserialized);
/// # }
/// ```
///
/// ## XML structure with varying keys
///
/// With `serde_xml_rs` tuple and struct variants are not supported since they fail to roundtrip.
/// The enum may have such variants as long as they are not serialized or deserialized.
///
/// ```
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// use serde_with::{serde_as, EnumMap};
///
/// # #[derive(Debug, Clone, PartialEq, Eq)]
/// #[derive(Serialize, Deserialize)]
/// enum EnumValue {
///     Int(i32),
///     String(String),
///     Unit,
/// }
///
/// #[serde_as]
/// # #[derive(Debug, Clone, PartialEq, Eq)]
/// #[derive(Serialize, Deserialize)]
/// struct VecEnumValues {
///     #[serde_as(as = "EnumMap")]
///     vec: Vec<EnumValue>,
/// }
///
/// // ---
///
/// // This will serialize this list of values
/// let values = VecEnumValues {
///     vec: vec![
///         EnumValue::Int(123),
///         EnumValue::String("FooBar".to_string()),
///         EnumValue::Int(456),
///         EnumValue::String("XXX".to_string()),
///         EnumValue::Unit,
///     ],
/// };
///
/// // into this XML document
/// // Duplicate keys are emitted for identical enum variants.
/// let expected = r#"
/// <?xml version="1.0" encoding="utf-8"?>
/// <VecEnumValues>
///     <vec>
///         <Int>123</Int>
///         <String>FooBar</String>
///         <Int>456</Int>
///         <String>XXX</String>
///         <Unit />
///     </vec>
/// </VecEnumValues>"#
/// // Remove whitespace
/// .replace("    ", "")
/// .replace('\n', "");
///
/// // Both serialization and deserialization work flawlessly.
/// let serialized = serde_xml_rs::to_string(&values).unwrap();
/// assert_eq!(expected, serialized);
/// let deserialized: VecEnumValues = serde_xml_rs::from_str(&serialized).unwrap();
/// assert_eq!(values, deserialized);
/// # }
/// ```
pub struct EnumMap;

impl<T> SerializeAs<Vec<T>> for EnumMap
where
    T: Serialize,
{
    fn serialize_as<S>(source: &Vec<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source.serialize(SeqAsMapSerializer(serializer))
    }
}

impl<'de, T> DeserializeAs<'de, Vec<T>> for EnumMap
where
    T: Deserialize<'de>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EnumMapVisitor<T> {
            is_human_readable: bool,
            phantom: PhantomData<T>,
        }

        impl<'de, T> Visitor<'de> for EnumMapVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = Vec<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map of enum values")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                Vec::deserialize(SeqDeserializer {
                    delegate: map,
                    is_human_readable: self.is_human_readable,
                })
            }
        }

        let is_human_readable = deserializer.is_human_readable();
        deserializer.deserialize_map(EnumMapVisitor {
            is_human_readable,
            phantom: PhantomData,
        })
    }
}

static END_OF_MAP_IDENTIFIER: &str = "__PRIVATE_END_OF_MAP_MARKER__";

// Serialization code below here

/// Convert a sequence to a map during serialization.
///
/// Only `serialize_seq` is implemented and forwarded to `serialize_map` on the inner `Serializer`.
/// The elements are serialized with [`SerializeSeqElement`].
struct SeqAsMapSerializer<S>(S);

impl<S> Serializer for SeqAsMapSerializer<S>
where
    S: Serializer,
{
    type Ok = S::Ok;
    type Error = S::Error;

    type SerializeSeq = SerializeSeqElement<S::SerializeMap>;
    type SerializeTuple = Impossible<S::Ok, S::Error>;
    type SerializeTupleStruct = Impossible<S::Ok, S::Error>;
    type SerializeTupleVariant = Impossible<S::Ok, S::Error>;
    type SerializeMap = Impossible<S::Ok, S::Error>;
    type SerializeStruct = Impossible<S::Ok, S::Error>;
    type SerializeStructVariant = Impossible<S::Ok, S::Error>;

    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_i128(self, _v: i128) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_u128(self, _v: u128) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_str(self, _v: &str) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        let is_human_readable = self.0.is_human_readable();
        self.0
            .serialize_map(len)
            .map(|delegate| SerializeSeqElement {
                delegate,
                is_human_readable,
            })
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }
}

/// Serialize a single element but turn the sequence into a map logic.
///
/// It uses [`EnumAsMapElementSerializer`] for the map element serialization.
///
/// The [`Serializer`] implementation handles all the `serialize_*_variant` functions and defers to [`SerializeVariant`] for the more complicated tuple and struct variants.
struct SerializeSeqElement<M> {
    delegate: M,
    is_human_readable: bool,
}

impl<M> SerializeSeq for SerializeSeqElement<M>
where
    M: SerializeMap,
{
    type Ok = M::Ok;
    type Error = M::Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(EnumAsMapElementSerializer {
            delegate: &mut self.delegate,
            is_human_readable: self.is_human_readable,
        })?;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.delegate.end()
    }
}

struct EnumAsMapElementSerializer<'a, M> {
    delegate: &'a mut M,
    is_human_readable: bool,
}

impl<'a, M> Serializer for EnumAsMapElementSerializer<'a, M>
where
    M: SerializeMap,
{
    type Ok = ();
    type Error = M::Error;

    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = SerializeVariant<'a, M>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = SerializeVariant<'a, M>;

    fn is_human_readable(&self) -> bool {
        self.is_human_readable
    }

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_i128(self, _v: i128) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_u128(self, _v: u128) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_str(self, _v: &str) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.delegate.serialize_entry(variant, &())?;
        Ok(())
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        self.delegate.serialize_entry(variant, value)?;
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(SerializeVariant {
            delegate: self.delegate,
            is_human_readable: self.is_human_readable,
            variant,
            content: Content::TupleStruct(name, Vec::with_capacity(len)),
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(SerError::custom("wrong type for EnumMap"))
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(SerializeVariant {
            delegate: self.delegate,
            is_human_readable: self.is_human_readable,
            variant,
            content: Content::Struct(name, Vec::with_capacity(len)),
        })
    }
}

/// Serialize a struct or tuple variant enum as a map element
///
/// [`SerializeStructVariant`] serializes a struct variant, and [`SerializeTupleVariant`] a tuple variant.
struct SerializeVariant<'a, M> {
    delegate: &'a mut M,
    is_human_readable: bool,
    variant: &'static str,
    content: Content,
}

impl<M> SerializeStructVariant for SerializeVariant<'_, M>
where
    M: SerializeMap,
{
    type Ok = ();

    type Error = M::Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        // Serialize to a Content type first
        let value: Content = value.serialize(ContentSerializer::new(self.is_human_readable))?;
        if let Content::Struct(_name, fields) = &mut self.content {
            fields.push((key, value));
        }
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.delegate.serialize_entry(&self.variant, &self.content)
    }
}

impl<M> SerializeTupleVariant for SerializeVariant<'_, M>
where
    M: SerializeMap,
{
    type Ok = ();

    type Error = M::Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        // Serialize to a Content type first
        let value: Content = value.serialize(ContentSerializer::new(self.is_human_readable))?;
        if let Content::TupleStruct(_name, fields) = &mut self.content {
            fields.push(value);
        }
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.delegate.serialize_entry(&self.variant, &self.content)
    }
}

// Below is deserialization code

/// Deserialize the sequence of enum instances.
///
/// The main [`Deserializer`] implementation handles the outer sequence (e.g., `Vec`), while the [`SeqAccess`] implementation is responsible for the inner elements.
struct SeqDeserializer<M> {
    delegate: M,
    is_human_readable: bool,
}

impl<'de, M> Deserializer<'de> for SeqDeserializer<M>
where
    M: MapAccess<'de>,
{
    type Error = M::Error;

    fn is_human_readable(&self) -> bool {
        self.is_human_readable
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_seq(self)
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de, M> SeqAccess<'de> for SeqDeserializer<M>
where
    M: MapAccess<'de>,
{
    type Error = M::Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        // We need to check if the map is done, or if there are still remaining elements.
        // But we cannot ask the MapAccess directly.
        // The only way is trying to deserialize, but we don't know the type yet.
        // So we assume there is a value and try to deserialize it.
        // If we later on find out that there is no value, we return a special error value, which we turn into `None`.
        match seed.deserialize(EnumDeserializer {
            delegate: &mut self.delegate,
            is_human_readable: self.is_human_readable,
        }) {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                // Unfortunately we loose the optional aspect of MapAccess, so we need to special case an error value to mark the end of the map.
                if err.to_string().contains(END_OF_MAP_IDENTIFIER) {
                    Ok(None)
                } else {
                    Err(err)
                }
            }
        }
    }

    fn size_hint(&self) -> Option<usize> {
        self.delegate.size_hint()
    }
}

/// Deserialize an enum from a map element
///
/// The [`Deserializer`] implementation is the starting point, which first calls the [`EnumAccess`] methods.
/// The [`EnumAccess`] is used to deserialize the enum variant type of the enum.
/// The [`VariantAccess`] is used to deserialize the value part of the enum.
struct EnumDeserializer<M> {
    delegate: M,
    is_human_readable: bool,
}

impl<'de, M> Deserializer<'de> for EnumDeserializer<M>
where
    M: MapAccess<'de>,
{
    type Error = M::Error;

    fn is_human_readable(&self) -> bool {
        self.is_human_readable
    }

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_enum("", &[], visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(self)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }
}

impl<'de, M> EnumAccess<'de> for EnumDeserializer<M>
where
    M: MapAccess<'de>,
{
    type Error = M::Error;
    type Variant = Self;

    fn variant_seed<T>(mut self, seed: T) -> Result<(T::Value, Self::Variant), Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.delegate.next_key_seed(seed)? {
            Some(key) => Ok((key, self)),

            // Unfortunately we loose the optional aspect of MapAccess, so we need to special case an error value to mark the end of the map.
            None => Err(DeError::custom(END_OF_MAP_IDENTIFIER)),
        }
    }
}

impl<'de, M> VariantAccess<'de> for EnumDeserializer<M>
where
    M: MapAccess<'de>,
{
    type Error = M::Error;

    fn unit_variant(mut self) -> Result<(), Self::Error> {
        self.delegate.next_value()
    }

    fn newtype_variant_seed<T>(mut self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        self.delegate.next_value_seed(seed)
    }

    fn tuple_variant<V>(mut self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate
            .next_value_seed(SeedTupleVariant { len, visitor })
    }

    fn struct_variant<V>(
        mut self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate.next_value_seed(SeedStructVariant { visitor })
    }
}

struct SeedTupleVariant<V> {
    len: usize,
    visitor: V,
}

impl<'de, V> DeserializeSeed<'de> for SeedTupleVariant<V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_tuple(self.len, self.visitor)
    }
}

struct SeedStructVariant<V> {
    visitor: V,
}

impl<'de, V> DeserializeSeed<'de> for SeedStructVariant<V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(self.visitor)
    }
}
