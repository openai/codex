use crate::prelude::*;

/// Serialize with an added suffix on every field name and deserialize by
/// trimming away the suffix.
///
/// You can set the visibility of the generated module by suffixing the module name with a module visibility.
/// `with_suffix!(pub(crate) suffix_foo "_foo");` creates a module with `pub(crate)` visibility.
/// The visibility is optional and by default `pub(self)`, i.e., private visibility is assumed.
///
/// **Note:** Use of this macro is incompatible with applying the [`deny_unknown_fields`] attribute
/// on the container.
/// While deserializing, it will always warn about unknown fields, even though they are processed
/// by the `with_suffix` wrapper.
/// More details can be found in [this issue][issue-with_suffix-deny_unknown_fields].
///
/// # Example
///
/// [Factorio Prototype tables] like to use suffixes to group related fields. In
/// simplified form, their JSON representation may resemble the following:
///
/// [Factorio Prototype tables]: https://lua-api.factorio.com/2.0.13/types/PipePictures.html
///
/// ```json
/// {
///   "frames": 4,
///   "spritesheet": "normal",
///   "frames_frozen": 1,
///   "spritesheet_frozen": "frozen",
///   "frames_visualization": 2,
///   "spritesheet_visualization": "vis",
/// }
/// ```
///
/// In Rust, we would ideally like to model this data as a couple of `SpriteData`
/// structs, rather than repeating the fields of `SpriteData` for each suffix.
///
/// ```rust
/// # #[allow(dead_code)]
/// struct Graphics {
///     normal: SpriteData,
///     frozen: SpriteData,
///     visualization: SpriteData,
/// }
///
/// # #[allow(dead_code)]
/// struct SpriteData {
///     frames: u64,
///     spritesheet: String,
/// }
/// ```
///
/// This `with_suffix!` macro produces an adapter that adds a suffix onto field
/// names during serialization and trims away the suffix during deserialization.
/// An implementation for the mentioned situation would use `with_suffix!` like this:
///
/// ```rust
/// use serde::{Deserialize, Serialize};
/// use serde_with::with_suffix;
///
/// #[derive(Serialize, Deserialize)]
/// struct Graphics {
///     #[serde(flatten)]
///     normal: SpriteData,
///     #[serde(flatten, with = "suffix_frozen")]
///     frozen: SpriteData,
///     #[serde(flatten, with = "suffix_visualization")]
///     visualization: SpriteData,
/// }
///
/// #[derive(Serialize, Deserialize)]
/// struct SpriteData {
///     frames: u64,
///     spritesheet: String,
/// }
///
/// with_suffix!(suffix_frozen "_frozen");
/// // You can also set the visibility of the generated suffix module, the default is private.
/// with_suffix!(pub suffix_visualization "_visualization");
/// #
/// # const EXPECTED: &str = r#"{
/// #   "frames": 4,
/// #   "spritesheet": "normal",
/// #   "frames_frozen": 1,
/// #   "spritesheet_frozen": "frozen",
/// #   "frames_visualization": 2,
/// #   "spritesheet_visualization": "vis"
/// # }"#;
///
/// fn main() {
///     let g = Graphics {
///         normal: SpriteData {
///             frames: 4,
///             spritesheet: "normal".to_owned(),
///         },
///         frozen: SpriteData {
///             frames: 1,
///             spritesheet: "frozen".to_owned(),
///         },
///         visualization: SpriteData {
///             frames: 2,
///             spritesheet: "vis".to_owned(),
///         },
///     };
///
///     let j = serde_json::to_string_pretty(&g).unwrap();
///     println!("{}", j);
/// #
/// #     assert_eq!(j, EXPECTED);
/// }
/// ```
///
/// [`deny_unknown_fields`]: https://serde.rs/container-attrs.html#deny_unknown_fields
/// [issue-with_suffix-deny_unknown_fields]: https://github.com/jonasbb/serde_with/issues/57
#[macro_export]
macro_rules! with_suffix {
    ($module:ident $suffix:expr) => {$crate::with_suffix!(pub(self) $module $suffix);};
    ($vis:vis $module:ident $suffix:expr) => {
        $vis mod $module {
            use $crate::serde::{Deserialize, Deserializer, Serialize, Serializer};
            use $crate::with_suffix::WithSuffix;

            #[allow(dead_code)]
            pub fn serialize<T, S>(object: &T, serializer: S) -> $crate::__private__::Result<S::Ok, S::Error>
            where
                T: Serialize,
                S: Serializer,
            {
                object.serialize(WithSuffix {
                    delegate: serializer,
                    suffix: $suffix,
                })
            }

            #[allow(dead_code)]
            pub fn deserialize<'de, T, D>(deserializer: D) -> $crate::__private__::Result<T, D::Error>
            where
                T: Deserialize<'de>,
                D: Deserializer<'de>,
            {
                T::deserialize(WithSuffix {
                    delegate: deserializer,
                    suffix: $suffix,
                })
            }
        }
    };
}

pub struct WithSuffix<'a, T> {
    pub delegate: T,
    pub suffix: &'a str,
}

impl<T> Serialize for WithSuffix<'_, T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.delegate.serialize(WithSuffix {
            delegate: serializer,
            suffix: self.suffix,
        })
    }
}

impl<'a, S> Serializer for WithSuffix<'a, S>
where
    S: Serializer,
{
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = WithSuffix<'a, S::SerializeMap>;
    type SerializeStruct = WithSuffix<'a, S::SerializeMap>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_u8(self, _v: u8) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_u64(self, _v: u64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.delegate
            .collect_str(&format_args!("{}{}", v, self.suffix))
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.delegate.serialize_none()
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.delegate.serialize_some(&WithSuffix {
            delegate: value,
            suffix: self.suffix,
        })
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
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
        Err(SerError::custom("wrong type for with_suffix"))
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
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(SerError::custom("wrong type for with_suffix"))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(WithSuffix {
            delegate: self.delegate.serialize_map(len)?,
            suffix: self.suffix,
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
        Err(SerError::custom("wrong type for with_suffix"))
    }
}

impl<S> SerializeMap for WithSuffix<'_, S>
where
    S: SerializeMap,
{
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.delegate.serialize_key(&WithSuffix {
            delegate: key,
            suffix: self.suffix,
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
            &WithSuffix {
                delegate: key,
                suffix: self.suffix,
            },
            value,
        )
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.delegate.end()
    }
}

impl<S> SerializeStruct for WithSuffix<'_, S>
where
    S: SerializeMap,
{
    type Ok = S::Ok;
    type Error = S::Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        let mut suffixed_key = String::with_capacity(key.len() + self.suffix.len());
        suffixed_key.push_str(key);
        suffixed_key.push_str(self.suffix);
        self.delegate.serialize_entry(&suffixed_key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.delegate.end()
    }
}

impl<'de, T> DeserializeSeed<'de> for WithSuffix<'_, T>
where
    T: DeserializeSeed<'de>,
{
    type Value = T::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.delegate.deserialize(WithSuffix {
            delegate: deserializer,
            suffix: self.suffix,
        })
    }
}

impl<'de, D> Deserializer<'de> for WithSuffix<'_, D>
where
    D: Deserializer<'de>,
{
    type Error = D::Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate.deserialize_map(WithSuffix {
            delegate: visitor,
            suffix: self.suffix,
        })
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate.deserialize_any(WithSuffixOption {
            first_key: None,
            delegate: visitor,
            suffix: self.suffix,
        })
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.delegate.deserialize_identifier(WithSuffix {
            delegate: visitor,
            suffix: self.suffix,
        })
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple tuple_struct
        map struct enum ignored_any
    }
}

impl<'de, V> Visitor<'de> for WithSuffix<'_, V>
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
        self.delegate.visit_map(WithSuffix {
            delegate: map,
            suffix: self.suffix,
        })
    }
}

impl<'de, A> MapAccess<'de> for WithSuffix<'_, A>
where
    A: MapAccess<'de>,
{
    type Error = A::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        while let Some(s) = self.delegate.next_key::<String>()? {
            if let Some(without_suffix) = s.strip_suffix(self.suffix) {
                return seed
                    .deserialize(without_suffix.into_deserializer())
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

pub struct WithSuffixOption<'a, T> {
    first_key: Option<String>,
    delegate: T,
    suffix: &'a str,
}

impl<'de, V> Visitor<'de> for WithSuffixOption<'_, V>
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
            if s.ends_with(self.suffix) {
                return self.delegate.visit_some(WithSuffixOption {
                    first_key: Some(s),
                    delegate: map,
                    suffix: self.suffix,
                });
            }
            map.next_value::<IgnoredAny>()?;
        }
        self.delegate.visit_none()
    }
}

impl<'de, A> Deserializer<'de> for WithSuffixOption<'_, A>
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

impl<'de, A> MapAccess<'de> for WithSuffixOption<'_, A>
where
    A: MapAccess<'de>,
{
    type Error = A::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if let Some(s) = self.first_key.take() {
            let without_suffix = s[0..s.len() - self.suffix.len()].into_deserializer();
            return seed.deserialize(without_suffix).map(Some);
        }
        while let Some(s) = self.delegate.next_key::<String>()? {
            if let Some(without_suffix) = s.strip_suffix(self.suffix) {
                return seed
                    .deserialize(without_suffix.into_deserializer())
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
