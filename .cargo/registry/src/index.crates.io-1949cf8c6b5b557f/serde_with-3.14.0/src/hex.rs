//! De/Serialization of hexadecimal encoded bytes
//!
//! This modules is only available when using the `hex` feature of the crate.
//!
//! Please check the documentation on the [`Hex`] type for details.

use crate::prelude::*;

/// Serialize bytes as a hex string
///
/// The type serializes a sequence of bytes as a hexadecimal string.
/// It works on any type implementing `AsRef<[u8]>` for serialization and `TryFrom<Vec<u8>>` for deserialization.
///
/// The format type parameter specifies if the hex string should use lower- or uppercase characters.
/// Valid options are the types [`formats::Lowercase`] and [`formats::Uppercase`].
/// Deserialization always supports lower- and uppercase characters, even mixed in one string.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_json::json;
/// # use serde_with::serde_as;
/// #
/// #[serde_as]
/// # #[derive(Debug, PartialEq, Eq)]
/// #[derive(Deserialize, Serialize)]
/// struct BytesLowercase(
///     // Equivalent to serde_with::hex::Hex<serde_with::formats::Lowercase>
///     #[serde_as(as = "serde_with::hex::Hex")]
///     Vec<u8>
/// );
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq, Eq)]
/// #[derive(Deserialize, Serialize)]
/// struct BytesUppercase(
///     #[serde_as(as = "serde_with::hex::Hex<serde_with::formats::Uppercase>")]
///     Vec<u8>
/// );
///
/// let b = b"Hello World!";
///
/// // Hex with lowercase letters
/// assert_eq!(
///     json!("48656c6c6f20576f726c6421"),
///     serde_json::to_value(BytesLowercase(b.to_vec())).unwrap()
/// );
/// // Hex with uppercase letters
/// assert_eq!(
///     json!("48656C6C6F20576F726C6421"),
///     serde_json::to_value(BytesUppercase(b.to_vec())).unwrap()
/// );
///
/// // Serialization always work from lower- and uppercase characters, even mixed case.
/// assert_eq!(
///     BytesLowercase(vec![0x00, 0xaa, 0xbc, 0x99, 0xff]),
///     serde_json::from_value(json!("00aAbc99FF")).unwrap()
/// );
/// assert_eq!(
///     BytesUppercase(vec![0x00, 0xaa, 0xbc, 0x99, 0xff]),
///     serde_json::from_value(json!("00aAbc99FF")).unwrap()
/// );
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq, Eq)]
/// #[derive(Deserialize, Serialize)]
/// struct ByteArray(
///     // Equivalent to serde_with::hex::Hex<serde_with::formats::Lowercase>
///     #[serde_as(as = "serde_with::hex::Hex")]
///     [u8; 12]
/// );
///
/// let b = *b"Hello World!";
///
/// assert_eq!(
///     json!("48656c6c6f20576f726c6421"),
///     serde_json::to_value(ByteArray(b)).unwrap()
/// );
///
/// // Serialization always work from lower- and uppercase characters, even mixed case.
/// assert_eq!(
///     ByteArray([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0xaa, 0xbc, 0x99, 0xff]),
///     serde_json::from_value(json!("0011223344556677aAbc99FF")).unwrap()
/// );
///
/// // Remember that the conversion may fail. (The following errors are specific to fixed-size arrays)
/// let error_result: Result<ByteArray, _> = serde_json::from_value(json!("42")); // Too short
/// error_result.unwrap_err();
///
/// let error_result: Result<ByteArray, _> =
///     serde_json::from_value(json!("000000000000000000000000000000")); // Too long
/// error_result.unwrap_err();
/// # }
/// ```
pub struct Hex<FORMAT: formats::Format = formats::Lowercase>(PhantomData<FORMAT>);

impl<T> SerializeAs<T> for Hex<formats::Lowercase>
where
    T: AsRef<[u8]>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&::hex::encode(source))
    }
}

impl<T> SerializeAs<T> for Hex<formats::Uppercase>
where
    T: AsRef<[u8]>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&::hex::encode_upper(source))
    }
}

impl<'de, T, FORMAT> DeserializeAs<'de, T> for Hex<FORMAT>
where
    T: TryFrom<Vec<u8>>,
    FORMAT: formats::Format,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        <Cow<'de, str> as Deserialize<'de>>::deserialize(deserializer)
            .and_then(|s| ::hex::decode(&*s).map_err(DeError::custom))
            .and_then(|vec: Vec<u8>| {
                let length = vec.len();
                vec.try_into().map_err(|_e: T::Error| {
                    DeError::custom(format_args!(
                        "Can't convert a Byte Vector of length {length} to the output type."
                    ))
                })
            })
    }
}
