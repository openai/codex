//! De/Serialization of JSON
//!
//! This modules is only available when using the `json` feature of the crate.

use crate::prelude::*;

/// Serialize value as string containing JSON
///
/// *Note*: This type is not necessary for normal usage of serde with JSON.
/// It is only required if the serialized format contains a string, which itself contains JSON.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` decides to
/// fail, or if `T` contains a map with non-string keys.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_with::{serde_as, json::JsonString};
/// #
/// #[serde_as]
/// #[derive(Deserialize, Serialize)]
/// struct A {
///     #[serde_as(as = "JsonString")]
///     other_struct: B,
/// }
/// #[derive(Deserialize, Serialize)]
/// struct B {
///     value: usize,
/// }
///
/// let v: A = serde_json::from_str(r#"{"other_struct":"{\"value\":5}"}"#).unwrap();
/// assert_eq!(5, v.other_struct.value);
///
/// let x = A {
///     other_struct: B { value: 10 },
/// };
/// assert_eq!(
///     r#"{"other_struct":"{\"value\":10}"}"#,
///     serde_json::to_string(&x).unwrap()
/// );
/// # }
/// ```
///
/// The `JsonString` converter takes a type argument, which allows altering the serialization behavior of the inner value, before it gets turned into a JSON string.
///
/// ```
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_with::{serde_as, json::JsonString};
/// # use std::collections::BTreeMap;
/// #
/// #[serde_as]
/// #[derive(Debug, Serialize, Deserialize, PartialEq)]
/// struct Struct {
///     #[serde_as(as = "JsonString<Vec<(JsonString, _)>>")]
///     value: BTreeMap<[u8; 2], u32>,
/// }
///
/// let value = Struct {
///     value: BTreeMap::from([([1, 2], 3), ([4, 5], 6)]),
/// };
/// assert_eq!(
///     r#"{"value":"[[\"[1,2]\",3],[\"[4,5]\",6]]"}"#,
///     serde_json::to_string(&value).unwrap()
/// );
/// # }
/// ```
pub struct JsonString<T = Same>(PhantomData<T>);

impl<T, TAs> SerializeAs<T> for JsonString<TAs>
where
    TAs: SerializeAs<T>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(
            &serde_json::to_string(&SerializeAsWrap::<T, TAs>::new(source))
                .map_err(SerError::custom)?,
        )
    }
}

impl<'de, T, TAs> DeserializeAs<'de, T> for JsonString<TAs>
where
    TAs: for<'a> DeserializeAs<'a, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper<S, SAs>(PhantomData<(S, SAs)>);

        impl<S, SAs> Visitor<'_> for Helper<S, SAs>
        where
            SAs: for<'a> DeserializeAs<'a, S>,
        {
            type Value = S;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("valid json object")
            }

            fn visit_str<E>(self, value: &str) -> Result<S, E>
            where
                E: DeError,
            {
                serde_json::from_str(value)
                    .map(DeserializeAsWrap::<S, SAs>::into_inner)
                    .map_err(DeError::custom)
            }
        }

        deserializer.deserialize_str(Helper::<T, TAs>(PhantomData))
    }
}
