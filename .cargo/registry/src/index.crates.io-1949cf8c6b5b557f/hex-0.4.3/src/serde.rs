//! Hex encoding with `serde`.
#[cfg_attr(
    all(feature = "alloc", feature = "serde"),
    doc = r##"
# Example

```
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Foo {
    #[serde(with = "hex")]
    bar: Vec<u8>,
}
```
"##
)]
use serde::de::{Error, Visitor};
use serde::Deserializer;
#[cfg(feature = "alloc")]
use serde::Serializer;

#[cfg(feature = "alloc")]
use alloc::string::String;

use core::fmt;
use core::marker::PhantomData;

use crate::FromHex;

#[cfg(feature = "alloc")]
use crate::ToHex;

/// Serializes `data` as hex string using uppercase characters.
///
/// Apart from the characters' casing, this works exactly like `serialize()`.
#[cfg(feature = "alloc")]
pub fn serialize_upper<S, T>(data: T, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: ToHex,
{
    let s = data.encode_hex_upper::<String>();
    serializer.serialize_str(&s)
}

/// Serializes `data` as hex string using lowercase characters.
///
/// Lowercase characters are used (e.g. `f9b4ca`). The resulting string's length
/// is always even, each byte in data is always encoded using two hex digits.
/// Thus, the resulting string contains exactly twice as many bytes as the input
/// data.
#[cfg(feature = "alloc")]
pub fn serialize<S, T>(data: T, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: ToHex,
{
    let s = data.encode_hex::<String>();
    serializer.serialize_str(&s)
}

/// Deserializes a hex string into raw bytes.
///
/// Both, upper and lower case characters are valid in the input string and can
/// even be mixed (e.g. `f9b4ca`, `F9B4CA` and `f9B4Ca` are all valid strings).
pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromHex,
    <T as FromHex>::Error: fmt::Display,
{
    struct HexStrVisitor<T>(PhantomData<T>);

    impl<'de, T> Visitor<'de> for HexStrVisitor<T>
    where
        T: FromHex,
        <T as FromHex>::Error: fmt::Display,
    {
        type Value = T;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "a hex encoded string")
        }

        fn visit_str<E>(self, data: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            FromHex::from_hex(data).map_err(Error::custom)
        }

        fn visit_borrowed_str<E>(self, data: &'de str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            FromHex::from_hex(data).map_err(Error::custom)
        }
    }

    deserializer.deserialize_str(HexStrVisitor(PhantomData))
}
