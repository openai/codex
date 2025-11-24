use crate::{
    content::{
        de::{Content as DeContent, ContentDeserializer},
        ser::{Content as SerContent, ContentSerializer},
    },
    prelude::*,
};

/// Convert `Vec` elements into key-value map entries
///
/// This maps a single struct/tuple/etc. to a map entry.
/// The map key is converted to a struct field.
/// The other values will be mapped to the map value.
///
/// The conversion supports structs, tuple structs, tuples, maps, and sequences.
/// Structs need a field that is named `$key$` to be used as the map key.
/// This can be done with the `#[serde(rename = "$key$")]` attribute.
/// Maps similarly need a map-key that is named `$key$`.
/// For tuples, tuple structs, and sequences the first element is used as the map key.
///
/// # Examples
///
/// ## Struct with String key in JSON
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// use serde_with::{serde_as, KeyValueMap};
///
/// # #[derive(Debug, Clone, PartialEq, Eq)]
/// #[derive(Serialize, Deserialize)]
/// struct SimpleStruct {
///     b: bool,
///     // The field named `$key$` will become the map key
///     #[serde(rename = "$key$")]
///     id: String,
///     i: i32,
/// }
///
/// #[serde_as]
/// # #[derive(Debug, Clone, PartialEq, Eq)]
/// #[derive(Serialize, Deserialize)]
/// struct KVMap(
///     #[serde_as(as = "KeyValueMap<_>")]
///     Vec<SimpleStruct>,
/// );
///
/// // ---
///
/// // This will serialize this list of values
/// let values = KVMap(vec![
///     SimpleStruct {
///         b: false,
///         id: "id-0000".to_string(),
///         i: 123,
///     },
///     SimpleStruct {
///         b: true,
///         id: "id-0001".to_string(),
///         i: 555,
///     },
///     SimpleStruct {
///         b: false,
///         id: "id-0002".to_string(),
///         i: 987,
///     },
/// ]);
///
/// // into this JSON map
/// let expected =
/// r#"{
///   "id-0000": {
///     "b": false,
///     "i": 123
///   },
///   "id-0001": {
///     "b": true,
///     "i": 555
///   },
///   "id-0002": {
///     "b": false,
///     "i": 987
///   }
/// }"#;
///
/// // Both serialization and deserialization work flawlessly.
/// let serialized = serde_json::to_string_pretty(&values).unwrap();
/// assert_eq!(expected, serialized);
/// let deserialized: KVMap = serde_json::from_str(&serialized).unwrap();
/// assert_eq!(values, deserialized);
/// # }
/// ```
///
/// ## Tuple struct with complex key in YAML
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// use serde_with::{serde_as, KeyValueMap};
/// use std::net::IpAddr;
/// # use std::str::FromStr;
///
/// # #[derive(Debug, Clone, PartialEq, Eq)]
/// #[derive(Serialize, Deserialize)]
/// struct TupleStruct (
///     // The first element in a tuple struct, tuple, or sequence becomes the map key
///     (IpAddr, u8),
///     bool,
/// );
///
/// #[serde_as]
/// # #[derive(Debug, Clone, PartialEq, Eq)]
/// #[derive(Serialize, Deserialize)]
/// struct KVMap(
///     #[serde_as(as = "KeyValueMap<_>")]
///     Vec<TupleStruct>,
/// );
///
/// // ---
///
/// // This will serialize this list of values
/// let values = KVMap(vec![
///     TupleStruct(
///         (IpAddr::from_str("127.0.0.1").unwrap(), 8),
///         true
///     ),
///     TupleStruct(
///         (IpAddr::from_str("::1").unwrap(), 128),
///         true
///     ),
///     TupleStruct(
///         (IpAddr::from_str("198.51.100.0").unwrap(), 24),
///         true
///     ),
/// ]);
///
/// // into this YAML
/// let expected =
/// r#"? - 127.0.0.1
///   - 8
/// : - true
/// ? - ::1
///   - 128
/// : - true
/// ? - 198.51.100.0
///   - 24
/// : - true
/// "#;
///
/// // Both serialization and deserialization work flawlessly.
/// let serialized = serde_yaml::to_string(&values).unwrap();
/// assert_eq!(expected, serialized);
/// let deserialized: KVMap = serde_yaml::from_str(&serialized).unwrap();
/// assert_eq!(values, deserialized);
/// # }
/// ```
pub struct KeyValueMap<T>(PhantomData<T>);

impl<T, TAs> SerializeAs<Vec<T>> for KeyValueMap<TAs>
where
    TAs: SerializeAs<T>,
{
    fn serialize_as<S>(source: &Vec<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        <Vec<TAs>>::serialize_as(source, SeqAsMapSerializer(serializer))
    }
}

impl<'de, T, TAs> DeserializeAs<'de, Vec<T>> for KeyValueMap<TAs>
where
    TAs: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct KeyValueMapVisitor<T, TAs> {
            is_human_readable: bool,
            phantom: PhantomData<(T, TAs)>,
        }

        impl<'de, T, TAs> Visitor<'de> for KeyValueMapVisitor<T, TAs>
        where
            TAs: DeserializeAs<'de, T>,
        {
            type Value = Vec<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                <Vec<TAs>>::deserialize_as(SeqDeserializer {
                    delegate: map,
                    is_human_readable: self.is_human_readable,
                })
            }
        }

        let is_human_readable = deserializer.is_human_readable();
        deserializer.deserialize_map(KeyValueMapVisitor::<T, TAs> {
            is_human_readable,
            phantom: PhantomData,
        })
    }
}

// TODO Replace this with a const generic string once adt_const_params is stable.
// This will allow something like this.
// The `"id"` part is the field name, which gets converted to/from the map key.
// #[serde_as(as = r#"KeyValueMap<"id", _>"#)]
// Vec<SimpleStruct>,
static MAP_KEY_IDENTIFIER: &str = "$key$";

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
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_i128(self, _v: i128) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_u128(self, _v: u128) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_str(self, _v: &str) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(SerError::custom("wrong type for KeyValueMap"))
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
        Err(SerError::custom("wrong type for KeyValueMap"))
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
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }
}

/// Serialize a single element but turn the sequence into a map logic.
///
/// It uses [`ElementAsKeyValueSerializer`] for the map element serialization.
///
/// The [`Serializer`] implementation handles `serialize_struct`, `serialize_map` and `serialize_seq` functions by deferring the work to [`SerializeStruct`], [`SerializeMap`] and [`SerializeSeq`] respectively.
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
        value.serialize(ElementAsKeyValueSerializer {
            delegate: &mut self.delegate,
            is_human_readable: self.is_human_readable,
        })?;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.delegate.end()
    }
}

struct ElementAsKeyValueSerializer<'a, M> {
    delegate: &'a mut M,
    is_human_readable: bool,
}

impl<'a, M> Serializer for ElementAsKeyValueSerializer<'a, M>
where
    M: SerializeMap,
{
    type Ok = ();
    type Error = M::Error;

    type SerializeSeq = KeyValueSeqSerializer<'a, M>;
    type SerializeTuple = KeyValueTupleSerializer<'a, M>;
    type SerializeTupleStruct = KeyValueTupleStructSerializer<'a, M>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = KeyValueMapSerializer<'a, M>;
    type SerializeStruct = KeyValueStructSerializer<'a, M>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    fn is_human_readable(&self) -> bool {
        self.is_human_readable
    }

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_i128(self, _v: i128) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_u128(self, _v: u128) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_str(self, _v: &str) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_some<T>(self, _value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(SerError::custom("wrong type for KeyValueMap"))
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
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(KeyValueSeqSerializer {
            delegate: self.delegate,
            is_human_readable: self.is_human_readable,
            content: Vec::with_capacity(len.unwrap_or(17) - 1),
            key: None,
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(KeyValueTupleSerializer {
            delegate: self.delegate,
            is_human_readable: self.is_human_readable,
            content: Vec::with_capacity(len - 1),
            key: None,
        })
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(KeyValueTupleStructSerializer {
            delegate: self.delegate,
            is_human_readable: self.is_human_readable,
            name,
            content: Vec::with_capacity(len - 1),
            key: None,
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(KeyValueMapSerializer {
            delegate: self.delegate,
            is_human_readable: self.is_human_readable,
            content: Vec::with_capacity(len.unwrap_or(17) - 1),
            next_is_magic_key: false,
            key: None,
            tmp: None,
        })
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(KeyValueStructSerializer {
            delegate: self.delegate,
            is_human_readable: self.is_human_readable,
            name,
            content: Vec::with_capacity(len - 1),
            key: None,
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(SerError::custom("wrong type for KeyValueMap"))
    }
}

/// Serialize a sequence to a key and value pair of a map.
///
/// This requires that the sequence has at least one element.
struct KeyValueSeqSerializer<'a, M> {
    delegate: &'a mut M,
    is_human_readable: bool,
    content: Vec<SerContent>,
    key: Option<SerContent>,
}

impl<M> SerializeSeq for KeyValueSeqSerializer<'_, M>
where
    M: SerializeMap,
{
    type Ok = ();
    type Error = M::Error;

    fn serialize_element<T>(&mut self, element: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let element: SerContent =
            element.serialize(ContentSerializer::new(self.is_human_readable))?;
        if self.key.is_none() {
            self.key = Some(element);
            return Ok(());
        }
        self.content.push(element);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if let Some(key) = self.key {
            self.delegate
                .serialize_entry(&key, &SerContent::Seq(self.content))
        } else {
            Err(SerError::custom("missing value for `$key$` field"))
        }
    }
}

/// Serialize a tuple to a key and value pair of a map.
///
/// This requires that the tuple has at least one element.
struct KeyValueTupleSerializer<'a, M> {
    delegate: &'a mut M,
    is_human_readable: bool,
    content: Vec<SerContent>,
    key: Option<SerContent>,
}

impl<M> SerializeTuple for KeyValueTupleSerializer<'_, M>
where
    M: SerializeMap,
{
    type Ok = ();
    type Error = M::Error;

    fn serialize_element<T>(&mut self, element: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let element: SerContent =
            element.serialize(ContentSerializer::new(self.is_human_readable))?;
        if self.key.is_none() {
            self.key = Some(element);
            return Ok(());
        }
        self.content.push(element);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if let Some(key) = self.key {
            self.delegate
                .serialize_entry(&key, &SerContent::Tuple(self.content))
        } else {
            Err(SerError::custom("missing value for `$key$` field"))
        }
    }
}

/// Serialize a tuple struct to a key and value pair of a map.
///
/// This requires that the tuple struct has at least one element.
struct KeyValueTupleStructSerializer<'a, M> {
    delegate: &'a mut M,
    is_human_readable: bool,
    name: &'static str,
    content: Vec<SerContent>,
    key: Option<SerContent>,
}

impl<M> SerializeTupleStruct for KeyValueTupleStructSerializer<'_, M>
where
    M: SerializeMap,
{
    type Ok = ();
    type Error = M::Error;

    fn serialize_field<T>(&mut self, field: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let field: SerContent = field.serialize(ContentSerializer::new(self.is_human_readable))?;
        if self.key.is_none() {
            self.key = Some(field);
            return Ok(());
        }
        self.content.push(field);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if let Some(key) = self.key {
            self.delegate
                .serialize_entry(&key, &SerContent::TupleStruct(self.name, self.content))
        } else {
            Err(SerError::custom("missing value for `$key$` field"))
        }
    }
}

/// Serialize a map to a key and value pair of a map.
///
/// This requires that the map has one element which serializes using the magic `$key$` key.
struct KeyValueMapSerializer<'a, M> {
    delegate: &'a mut M,
    is_human_readable: bool,
    content: Vec<(SerContent, SerContent)>,
    next_is_magic_key: bool,
    key: Option<SerContent>,
    tmp: Option<SerContent>,
}

impl<M> SerializeMap for KeyValueMapSerializer<'_, M>
where
    M: SerializeMap,
{
    type Ok = ();
    type Error = M::Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let key: SerContent = key.serialize(ContentSerializer::new(self.is_human_readable))?;
        if key.as_str() == Some(MAP_KEY_IDENTIFIER) {
            self.next_is_magic_key = true;
            return Ok(());
        }
        self.tmp = Some(key);
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let value: SerContent = value.serialize(ContentSerializer::new(self.is_human_readable))?;

        if self.next_is_magic_key {
            self.next_is_magic_key = false;
            self.key = Some(value);
            return Ok(());
        }
        self.content.push((
            self.tmp
                .take()
                .expect("serialize_value called before serialize_key"),
            value,
        ));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if let Some(key) = self.key {
            self.delegate
                .serialize_entry(&key, &SerContent::Map(self.content))
        } else {
            Err(SerError::custom("missing value for `$key$` field"))
        }
    }
}

/// Serialize a struct to a key and value pair of a map.
///
/// This requires that the struct has one field named `$key$`.
struct KeyValueStructSerializer<'a, M> {
    delegate: &'a mut M,
    is_human_readable: bool,
    name: &'static str,
    content: Vec<(&'static str, SerContent)>,
    key: Option<SerContent>,
}

impl<M> SerializeStruct for KeyValueStructSerializer<'_, M>
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
        let value: SerContent = value.serialize(ContentSerializer::new(self.is_human_readable))?;

        if key == MAP_KEY_IDENTIFIER {
            self.key = Some(value);
            return Ok(());
        }
        self.content.push((key, value));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if let Some(key) = self.key {
            self.delegate
                .serialize_entry(&key, &SerContent::Struct(self.name, self.content))
        } else {
            Err(SerError::custom("missing value for `$key$` field"))
        }
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
        let key_value: Option<DeContent<'de>> = self.delegate.next_key()?;
        if let Some(key_value) = key_value {
            seed.deserialize(MapKeyDeserializer {
                delegate: &mut self.delegate,
                is_human_readable: self.is_human_readable,
                key_value,
            })
            .map(Some)
        } else {
            Ok(None)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        self.delegate.size_hint()
    }
}

struct MapKeyDeserializer<'de, M> {
    delegate: M,
    is_human_readable: bool,
    key_value: DeContent<'de>,
}

impl<'de, M> Deserializer<'de> for MapKeyDeserializer<'de, M>
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
        self.deserialize_map(visitor)
    }

    fn deserialize_seq<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate.next_value_seed(KeyValueSeqDeserialize {
            delegate: visitor,
            first: Some(self.key_value),
        })
    }

    fn deserialize_tuple<V>(mut self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate.next_value_seed(KeyValueTupleDeserialize {
            delegate: visitor,
            len,
            first: Some(self.key_value),
        })
    }

    fn deserialize_tuple_struct<V>(
        mut self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate
            .next_value_seed(KeyValueTupleStructDeserialize {
                delegate: visitor,
                name,
                len,
                first: Some(self.key_value),
            })
    }

    fn deserialize_map<V>(mut self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate.next_value_seed(KeyValueMapDeserialize {
            delegate: visitor,
            first: Some(self.key_value),
        })
    }

    fn deserialize_struct<V>(
        mut self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate.next_value_seed(KeyValueStructDeserialize {
            delegate: visitor,
            name,
            fields,
            first: Some(self.key_value),
        })
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct
        enum identifier ignored_any
    }
}

struct KeyValueSeqDeserialize<'de, V> {
    delegate: V,
    first: Option<DeContent<'de>>,
}

impl<'de, V> DeserializeSeed<'de> for KeyValueSeqDeserialize<'de, V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn deserialize<D>(mut self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let is_human_readable = deserializer.is_human_readable();
        deserializer.deserialize_seq(VisitorWrapper {
            delegate: self.delegate,
            is_human_readable,
            first: self.first.take(),
        })
    }
}

struct KeyValueTupleDeserialize<'de, V> {
    delegate: V,
    len: usize,
    first: Option<DeContent<'de>>,
}

impl<'de, V> DeserializeSeed<'de> for KeyValueTupleDeserialize<'de, V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn deserialize<D>(mut self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let is_human_readable = deserializer.is_human_readable();
        deserializer.deserialize_tuple(
            self.len,
            VisitorWrapper {
                delegate: self.delegate,
                is_human_readable,
                first: self.first.take(),
            },
        )
    }
}

struct KeyValueTupleStructDeserialize<'de, V> {
    delegate: V,
    name: &'static str,
    len: usize,
    first: Option<DeContent<'de>>,
}

impl<'de, V> DeserializeSeed<'de> for KeyValueTupleStructDeserialize<'de, V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn deserialize<D>(mut self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let is_human_readable = deserializer.is_human_readable();
        deserializer.deserialize_tuple_struct(
            self.name,
            self.len,
            VisitorWrapper {
                delegate: self.delegate,
                is_human_readable,
                first: self.first.take(),
            },
        )
    }
}

struct KeyValueMapDeserialize<'de, V> {
    delegate: V,
    first: Option<DeContent<'de>>,
}

impl<'de, V> DeserializeSeed<'de> for KeyValueMapDeserialize<'de, V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn deserialize<D>(mut self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let is_human_readable = deserializer.is_human_readable();
        deserializer.deserialize_map(VisitorWrapper {
            delegate: self.delegate,
            is_human_readable,
            first: self.first.take(),
        })
    }
}

struct KeyValueStructDeserialize<'de, V> {
    delegate: V,
    name: &'static str,
    fields: &'static [&'static str],
    first: Option<DeContent<'de>>,
}

impl<'de, V> DeserializeSeed<'de> for KeyValueStructDeserialize<'de, V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn deserialize<D>(mut self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let is_human_readable = deserializer.is_human_readable();
        deserializer.deserialize_struct(
            self.name,
            self.fields,
            VisitorWrapper {
                delegate: self.delegate,
                is_human_readable,
                first: self.first.take(),
            },
        )
    }
}

struct VisitorWrapper<'de, V> {
    delegate: V,
    is_human_readable: bool,
    first: Option<DeContent<'de>>,
}

impl<'de, V> Visitor<'de> for VisitorWrapper<'de, V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.delegate.expecting(formatter)
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        self.delegate.visit_map(MapAccessWrapper {
            delegate: map,
            is_human_readable: self.is_human_readable,
            first: self.first,
        })
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        self.delegate.visit_seq(SeqAccessWrapper {
            delegate: seq,
            is_human_readable: self.is_human_readable,
            first: self.first,
        })
    }
}

struct MapAccessWrapper<'de, M> {
    delegate: M,
    is_human_readable: bool,
    first: Option<DeContent<'de>>,
}

impl<'de, M> MapAccess<'de> for MapAccessWrapper<'de, M>
where
    M: MapAccess<'de>,
{
    type Error = M::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if self.first.is_some() {
            seed.deserialize(serde::de::value::StringDeserializer::new(
                MAP_KEY_IDENTIFIER.to_string(),
            ))
            .map(Some)
        } else {
            self.delegate.next_key_seed(seed)
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        if let Some(first) = self.first.take() {
            seed.deserialize(ContentDeserializer::new(first, self.is_human_readable))
        } else {
            self.delegate.next_value_seed(seed)
        }
    }
}

struct SeqAccessWrapper<'de, M> {
    delegate: M,
    is_human_readable: bool,
    first: Option<DeContent<'de>>,
}

impl<'de, S> SeqAccess<'de> for SeqAccessWrapper<'de, S>
where
    S: SeqAccess<'de>,
{
    type Error = S::Error;

    fn next_element_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if let Some(first) = self.first.take() {
            seed.deserialize(ContentDeserializer::new(first, self.is_human_readable))
                .map(Some)
        } else {
            self.delegate.next_element_seed(seed)
        }
    }
}
