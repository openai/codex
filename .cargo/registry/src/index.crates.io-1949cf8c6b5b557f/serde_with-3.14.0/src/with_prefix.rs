use crate::prelude::*;

/// Serialize with an added prefix on every field name and deserialize by
/// trimming away the prefix.
///
/// You can set the visibility of the generated module by prefixing the module name with a module visibility.
/// `with_prefix!(pub(crate) prefix_foo "foo_");` creates a module with `pub(crate)` visibility.
/// The visibility is optional and by default `pub(self)`, i.e., private visibility is assumed.
///
/// **Note:** Use of this macro is incompatible with applying the [`deny_unknown_fields`] attribute
/// on the container.
/// While deserializing, it will always warn about unknown fields, even though they are processed
/// by the `with_prefix` wrapper.
/// More details can be found in [this issue][issue-with_prefix-deny_unknown_fields].
///
/// # Example
///
/// The [Challonge REST API] likes to use prefixes to group related fields. In
/// simplified form, their JSON may resemble the following:
///
/// [Challonge REST API]: https://api.challonge.com/v1/documents/matches/show
///
/// ```json
/// {
///   "player1_name": "name1",
///   "player1_votes": 1,
///   "player2_name": "name2",
///   "player2_votes": 2
/// }
/// ```
///
/// In Rust, we would ideally like to model this data as a pair of `Player`
/// structs, rather than repeating the fields of `Player` for each prefix.
///
/// ```rust
/// # #[allow(dead_code)]
/// struct Match {
///     player1: Player,
///     player2: Player,
/// }
///
/// # #[allow(dead_code)]
/// struct Player {
///     name: String,
///     votes: u64,
/// }
/// ```
///
/// This `with_prefix!` macro produces an adapter that adds a prefix onto field
/// names during serialization and trims away the prefix during deserialization.
/// An implementation of the Challonge API would use `with_prefix!` like this:
///
/// ```rust
/// use serde::{Deserialize, Serialize};
/// use serde_with::with_prefix;
///
/// #[derive(Serialize, Deserialize)]
/// struct Match {
///     #[serde(flatten, with = "prefix_player1")]
///     player1: Player,
///     #[serde(flatten, with = "prefix_player2")]
///     player2: Player,
/// }
///
/// #[derive(Serialize, Deserialize)]
/// struct Player {
///     name: String,
///     votes: u64,
/// }
///
/// with_prefix!(prefix_player1 "player1_");
/// // You can also set the visibility of the generated prefix module, the default is private.
/// with_prefix!(pub prefix_player2 "player2_");
/// #
/// # const EXPECTED: &str = r#"{
/// #   "player1_name": "name1",
/// #   "player1_votes": 1,
/// #   "player2_name": "name2",
/// #   "player2_votes": 2
/// # }"#;
///
/// fn main() {
///     let m = Match {
///         player1: Player {
///             name: "name1".to_owned(),
///             votes: 1,
///         },
///         player2: Player {
///             name: "name2".to_owned(),
///             votes: 2,
///         },
///     };
///
///     let j = serde_json::to_string_pretty(&m).unwrap();
///     println!("{}", j);
/// #
/// #     assert_eq!(j, EXPECTED);
/// }
/// ```
///
/// [`deny_unknown_fields`]: https://serde.rs/container-attrs.html#deny_unknown_fields
/// [issue-with_prefix-deny_unknown_fields]: https://github.com/jonasbb/serde_with/issues/57
#[macro_export]
macro_rules! with_prefix {
    ($module:ident $prefix:expr) => {$crate::with_prefix!(pub(self) $module $prefix);};
    ($vis:vis $module:ident $prefix:expr) => {
        $vis mod $module {
            use $crate::serde::{Deserialize, Deserializer, Serialize, Serializer};
            use $crate::with_prefix::WithPrefix;

            #[allow(dead_code)]
            pub fn serialize<T, S>(object: &T, serializer: S) -> $crate::__private__::Result<S::Ok, S::Error>
            where
                T: Serialize,
                S: Serializer,
            {
                object.serialize(WithPrefix {
                    delegate: serializer,
                    prefix: $prefix,
                })
            }

            #[allow(dead_code)]
            pub fn deserialize<'de, T, D>(deserializer: D) -> $crate::__private__::Result<T, D::Error>
            where
                T: Deserialize<'de>,
                D: Deserializer<'de>,
            {
                T::deserialize(WithPrefix {
                    delegate: deserializer,
                    prefix: $prefix,
                })
            }
        }
    };
}

pub struct WithPrefix<'a, T> {
    pub delegate: T,
    pub prefix: &'a str,
}

impl<T> Serialize for WithPrefix<'_, T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.delegate.serialize(WithPrefix {
            delegate: serializer,
            prefix: self.prefix,
        })
    }
}

impl<'a, S> Serializer for WithPrefix<'a, S>
where
    S: Serializer,
{
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = WithPrefix<'a, S::SerializeMap>;
    type SerializeStruct = WithPrefix<'a, S::SerializeMap>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.delegate
            .collect_str(&format_args!("{}{}", self.prefix, v))
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.delegate.serialize_none()
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.delegate.serialize_some(&WithPrefix {
            delegate: value,
            prefix: self.prefix,
        })
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(WithPrefix {
            delegate: self.delegate.serialize_map(len)?,
            prefix: self.prefix,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(SerError::custom("wrong type for with_prefix"))
    }
}

impl<S> SerializeMap for WithPrefix<'_, S>
where
    S: SerializeMap,
{
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.delegate.serialize_key(&WithPrefix {
            delegate: key,
            prefix: self.prefix,
        })
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.delegate.serialize_value(value)
    }

    fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<(), Self::Error>
    where
        K: ?Sized + Serialize,
        V: ?Sized + Serialize,
    {
        self.delegate.serialize_entry(
            &WithPrefix {
                delegate: key,
                prefix: self.prefix,
            },
            value,
        )
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.delegate.end()
    }
}

impl<S> SerializeStruct for WithPrefix<'_, S>
where
    S: SerializeMap,
{
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let mut prefixed_key = String::with_capacity(self.prefix.len() + key.len());
        prefixed_key.push_str(self.prefix);
        prefixed_key.push_str(key);
        self.delegate.serialize_entry(&prefixed_key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.delegate.end()
    }
}

impl<'de, T> DeserializeSeed<'de> for WithPrefix<'_, T>
where
    T: DeserializeSeed<'de>,
{
    type Value = T::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.delegate.deserialize(WithPrefix {
            delegate: deserializer,
            prefix: self.prefix,
        })
    }
}

impl<'de, D> Deserializer<'de> for WithPrefix<'_, D>
where
    D: Deserializer<'de>,
{
    type Error = D::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate.deserialize_map(WithPrefix {
            delegate: visitor,
            prefix: self.prefix,
        })
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate.deserialize_any(WithPrefixOption {
            first_key: None,
            delegate: visitor,
            prefix: self.prefix,
        })
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate.deserialize_identifier(WithPrefix {
            delegate: visitor,
            prefix: self.prefix,
        })
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple tuple_struct
        map struct enum ignored_any
    }
}

impl<'de, V> Visitor<'de> for WithPrefix<'_, V>
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
        self.delegate.visit_map(WithPrefix {
            delegate: map,
            prefix: self.prefix,
        })
    }
}

impl<'de, A> MapAccess<'de> for WithPrefix<'_, A>
where
    A: MapAccess<'de>,
{
    type Error = A::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        while let Some(s) = self.delegate.next_key::<String>()? {
            if let Some(without_prefix) = s.strip_prefix(self.prefix) {
                return seed
                    .deserialize(without_prefix.into_deserializer())
                    .map(Some);
            }
            self.delegate.next_value::<IgnoredAny>()?;
        }
        Ok(None)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        self.delegate.next_value_seed(seed)
    }
}

pub struct WithPrefixOption<'a, T> {
    first_key: Option<String>,
    delegate: T,
    prefix: &'a str,
}

impl<'de, V> Visitor<'de> for WithPrefixOption<'_, V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.delegate.expecting(formatter)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.delegate.visit_none()
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while let Some(s) = map.next_key::<String>()? {
            if s.starts_with(self.prefix) {
                return self.delegate.visit_some(WithPrefixOption {
                    first_key: Some(s),
                    delegate: map,
                    prefix: self.prefix,
                });
            }
            map.next_value::<IgnoredAny>()?;
        }
        self.delegate.visit_none()
    }
}

impl<'de, A> Deserializer<'de> for WithPrefixOption<'_, A>
where
    A: MapAccess<'de>,
{
    type Error = A::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

impl<'de, A> MapAccess<'de> for WithPrefixOption<'_, A>
where
    A: MapAccess<'de>,
{
    type Error = A::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if let Some(s) = self.first_key.take() {
            let without_prefix = s[self.prefix.len()..].into_deserializer();
            return seed.deserialize(without_prefix).map(Some);
        }
        while let Some(s) = self.delegate.next_key::<String>()? {
            if let Some(without_prefix) = s.strip_prefix(self.prefix) {
                return seed
                    .deserialize(without_prefix.into_deserializer())
                    .map(Some);
            }
            self.delegate.next_value::<IgnoredAny>()?;
        }
        Ok(None)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        self.delegate.next_value_seed(seed)
    }
}
