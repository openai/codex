use serde::de::value::SeqAccessDeserializer;
use serde::ser::{Impossible, SerializeStructVariant, SerializeTupleVariant};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;

/// Serde case-insensitive deserializer for an untagged `enum`.
///
/// This function converts values to lowercase before deserializing as the `enum`. Requires the
/// `#[serde(rename_all = "lowercase")]` attribute to be set on the `enum`.
///
/// # Example
///
/// In example below, the following JSON values all deserialize to
/// `GroceryBasket { fruit_item: Fruit::Banana }`:
///
///  * `{"fruit_item": "banana"}`
///  * `{"fruit_item": "BANANA"}`
///  * `{"fruit_item": "Banana"}`
///
/// Note: this example does not compile automatically due to
/// [Rust issue #29286](https://github.com/rust-lang/rust/issues/29286).
///
/// ```
/// # /*
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// #[serde(rename_all = "lowercase")]
/// enum Fruit {
///     Apple,
///     Banana,
///     Orange,
/// }
///
/// #[derive(Deserialize)]
/// struct GroceryBasket {
///     #[serde(deserialize_with = "helpers::deserialize_untagged_enum_case_insensitive")]
///     fruit_item: Fruit,
/// }
/// # */
/// ```
pub fn deserialize_untagged_enum_case_insensitive<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(Value::String(
        String::deserialize(deserializer)?.to_lowercase(),
    ))
    .map_err(serde::de::Error::custom)
}

/// Serde space-delimited string deserializer for a `Vec<String>`.
///
/// This function splits a JSON string at each space character into a `Vec<String>` .
///
/// # Example
///
/// In example below, the JSON value `{"items": "foo bar baz"}` would deserialize to:
///
/// ```
/// # struct GroceryBasket {
/// #     items: Vec<String>,
/// # }
/// GroceryBasket {
///     items: vec!["foo".to_string(), "bar".to_string(), "baz".to_string()]
/// };
/// ```
///
/// Note: this example does not compile automatically due to
/// [Rust issue #29286](https://github.com/rust-lang/rust/issues/29286).
///
/// ```
/// # /*
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct GroceryBasket {
///     #[serde(deserialize_with = "helpers::deserialize_space_delimited_vec")]
///     items: Vec<String>,
/// }
/// # */
/// ```
pub fn deserialize_space_delimited_vec<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    if let Some(space_delimited) = Option::<String>::deserialize(deserializer)? {
        let entries = space_delimited
            .split(' ')
            .map(|s| Value::String(s.to_string()))
            .collect();
        T::deserialize(Value::Array(entries)).map_err(serde::de::Error::custom)
    } else {
        // If the JSON value is null, use the default value.
        Ok(T::default())
    }
}

/// Deserializes a string or array of strings into an array of strings
pub fn deserialize_optional_string_or_vec_string<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrVec(PhantomData<Vec<String>>);

    impl<'de> serde::de::Visitor<'de> for StringOrVec {
        type Value = Option<Vec<String>>;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            formatter.write_str("string or list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(vec![value.to_owned()]))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_seq<S>(self, visitor: S) -> Result<Self::Value, S::Error>
        where
            S: serde::de::SeqAccess<'de>,
        {
            Deserialize::deserialize(SeqAccessDeserializer::new(visitor)).map(Some)
        }
    }

    deserializer.deserialize_any(StringOrVec(PhantomData))
}

/// Serde space-delimited string serializer for an `Option<Vec<String>>`.
///
/// This function serializes a string vector into a single space-delimited string.
/// If `string_vec_opt` is `None`, the function serializes it as `None` (e.g., `null`
/// in the case of JSON serialization).
pub fn serialize_space_delimited_vec<T, S>(
    vec_opt: &Option<Vec<T>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    T: AsRef<str>,
    S: Serializer,
{
    if let Some(ref vec) = *vec_opt {
        let space_delimited = vec.iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(" ");

        serializer.serialize_str(&space_delimited)
    } else {
        serializer.serialize_none()
    }
}

/// Serde string serializer for an enum.
///
/// Source:
/// [https://github.com/serde-rs/serde/issues/553](https://github.com/serde-rs/serde/issues/553)
pub fn variant_name<T: Serialize>(t: &T) -> &'static str {
    #[derive(Debug)]
    struct NotEnum;
    type Result<T> = std::result::Result<T, NotEnum>;
    impl Error for NotEnum {
        fn description(&self) -> &str {
            "not struct"
        }
    }
    impl Display for NotEnum {
        fn fmt(&self, _f: &mut Formatter) -> std::fmt::Result {
            unimplemented!()
        }
    }
    impl serde::ser::Error for NotEnum {
        fn custom<T: Display>(_msg: T) -> Self {
            NotEnum
        }
    }

    struct VariantName;
    impl Serializer for VariantName {
        type Ok = &'static str;
        type Error = NotEnum;
        type SerializeSeq = Impossible<Self::Ok, Self::Error>;
        type SerializeTuple = Impossible<Self::Ok, Self::Error>;
        type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
        type SerializeTupleVariant = Enum;
        type SerializeMap = Impossible<Self::Ok, Self::Error>;
        type SerializeStruct = Impossible<Self::Ok, Self::Error>;
        type SerializeStructVariant = Enum;
        fn serialize_bool(self, _v: bool) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_i8(self, _v: i8) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_i16(self, _v: i16) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_i32(self, _v: i32) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_i64(self, _v: i64) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_u8(self, _v: u8) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_u16(self, _v: u16) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_u32(self, _v: u32) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_u64(self, _v: u64) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_f32(self, _v: f32) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_f64(self, _v: f64) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_char(self, _v: char) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_str(self, _v: &str) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_none(self) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_unit(self) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_unit_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            variant: &'static str,
        ) -> Result<Self::Ok> {
            Ok(variant)
        }
        fn serialize_newtype_struct<T: ?Sized + Serialize>(
            self,
            _name: &'static str,
            _value: &T,
        ) -> Result<Self::Ok> {
            Err(NotEnum)
        }
        fn serialize_newtype_variant<T: ?Sized + Serialize>(
            self,
            _name: &'static str,
            _variant_index: u32,
            variant: &'static str,
            _value: &T,
        ) -> Result<Self::Ok> {
            Ok(variant)
        }
        fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
            Err(NotEnum)
        }
        fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
            Err(NotEnum)
        }
        fn serialize_tuple_struct(
            self,
            _name: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeTupleStruct> {
            Err(NotEnum)
        }
        fn serialize_tuple_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeTupleVariant> {
            Ok(Enum(variant))
        }
        fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
            Err(NotEnum)
        }
        fn serialize_struct(
            self,
            _name: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeStruct> {
            Err(NotEnum)
        }
        fn serialize_struct_variant(
            self,
            _name: &'static str,
            _variant_index: u32,
            variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeStructVariant> {
            Ok(Enum(variant))
        }
    }

    struct Enum(&'static str);
    impl SerializeStructVariant for Enum {
        type Ok = &'static str;
        type Error = NotEnum;
        fn serialize_field<T: ?Sized + Serialize>(
            &mut self,
            _key: &'static str,
            _value: &T,
        ) -> Result<()> {
            Ok(())
        }
        fn end(self) -> Result<Self::Ok> {
            Ok(self.0)
        }
    }
    impl SerializeTupleVariant for Enum {
        type Ok = &'static str;
        type Error = NotEnum;
        fn serialize_field<T: ?Sized + Serialize>(&mut self, _value: &T) -> Result<()> {
            Ok(())
        }
        fn end(self) -> Result<Self::Ok> {
            Ok(self.0)
        }
    }

    t.serialize(VariantName).unwrap()
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    #[derive(Deserialize, Debug, Clone)]
    pub struct ObjectWithOptionalStringOrVecString {
        #[serde(deserialize_with = "crate::helpers::deserialize_optional_string_or_vec_string")]
        pub strings: Option<Vec<String>>,
    }

    #[test]
    fn test_deserialize_optional_string_or_vec_string_none() {
        let list_of_strings: ObjectWithOptionalStringOrVecString =
            serde_json::from_str(r#"{ "strings": null }"#).unwrap();
        assert_eq!(None, list_of_strings.strings);
    }

    #[test]
    fn test_deserialize_optional_string_or_vec_string_single_value() {
        let list_of_strings: ObjectWithOptionalStringOrVecString =
            serde_json::from_str(r#"{ "strings": "v1" }"#).unwrap();
        assert_eq!(Some(vec!["v1".to_string()]), list_of_strings.strings);
    }

    #[test]
    fn test_deserialize_optional_string_or_vec_string_vec() {
        let list_of_strings: ObjectWithOptionalStringOrVecString =
            serde_json::from_str(r#"{ "strings": ["v1", "v2"] }"#).unwrap();
        assert_eq!(
            Some(vec!["v1".to_string(), "v2".to_string()]),
            list_of_strings.strings
        );
    }
}
