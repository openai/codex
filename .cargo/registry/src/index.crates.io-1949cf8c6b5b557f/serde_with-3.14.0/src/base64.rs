//! De/Serialization of base64 encoded bytes
//!
//! This modules is only available when using the `base64` feature of the crate.
//!
//! Please check the documentation on the [`Base64`] type for details.

use crate::prelude::*;

/// Serialize bytes with base64
///
/// The type serializes a sequence of bytes as a base64 string.
/// It works on any type implementing `AsRef<[u8]>` for serialization and `TryFrom<Vec<u8>>` for deserialization.
///
/// The type allows customizing the character set and the padding behavior.
/// The `ALPHABET` is a type implementing [`Alphabet`].
/// `PADDING` specifies if serializing should emit padding.
/// Deserialization always supports padded and unpadded formats.
/// [`formats::Padded`] emits padding and [`formats::Unpadded`] leaves it off.
///
/// ```rust
/// # #[cfg(feature = "macros")] {
/// # use serde::{Deserialize, Serialize};
/// # use serde_with::serde_as;
/// use serde_with::base64::{Base64, Bcrypt, BinHex, Standard};
/// use serde_with::formats::{Padded, Unpadded};
///
/// #[serde_as]
/// # #[derive(Debug, PartialEq, Eq)]
/// #[derive(Serialize, Deserialize)]
/// struct B64 {
///     // The default is the same as Standard character set with padding
///     #[serde_as(as = "Base64")]
///     default: Vec<u8>,
///     // Only change the character set, implies padding
///     #[serde_as(as = "Base64<BinHex>")]
///     charset_binhex: Vec<u8>,
///
///     #[serde_as(as = "Base64<Standard, Padded>")]
///     explicit_padding: Vec<u8>,
///     #[serde_as(as = "Base64<Bcrypt, Unpadded>")]
///     no_padding: Vec<u8>,
/// }
///
/// let b64 = B64 {
///     default: b"Hello World".to_vec(),
///     charset_binhex: b"Hello World".to_vec(),
///     explicit_padding: b"Hello World".to_vec(),
///     no_padding: b"Hello World".to_vec(),
/// };
/// let json = serde_json::json!({
///     "default": "SGVsbG8gV29ybGQ=",
///     "charset_binhex": "5'9XE'mJ9fpbE'3=",
///     "explicit_padding": "SGVsbG8gV29ybGQ=",
///     "no_padding": "QETqZE6eT07wZEO",
/// });
///
/// // Test serialization and deserialization
/// assert_eq!(json, serde_json::to_value(&b64).unwrap());
/// assert_eq!(b64, serde_json::from_value(json).unwrap());
/// # }
/// ```
// The padding might be better as `const PADDING: bool = true`
// https://blog.rust-lang.org/inside-rust/2021/09/06/Splitting-const-generics.html#featureconst_generics_default/
pub struct Base64<ALPHABET: Alphabet = Standard, PADDING: formats::Format = formats::Padded>(
    PhantomData<(ALPHABET, PADDING)>,
);

impl<T, ALPHABET> SerializeAs<T> for Base64<ALPHABET, formats::Padded>
where
    T: AsRef<[u8]>,
    ALPHABET: Alphabet,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use ::base64::Engine as _;

        ::base64::engine::GeneralPurpose::new(
            &ALPHABET::charset(),
            ::base64::engine::general_purpose::PAD,
        )
        .encode(source)
        .serialize(serializer)
    }
}

impl<T, ALPHABET> SerializeAs<T> for Base64<ALPHABET, formats::Unpadded>
where
    T: AsRef<[u8]>,
    ALPHABET: Alphabet,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use ::base64::Engine as _;

        ::base64::engine::GeneralPurpose::new(
            &ALPHABET::charset(),
            ::base64::engine::general_purpose::NO_PAD,
        )
        .encode(source)
        .serialize(serializer)
    }
}

// Our decoders uniformly do not care about padding.
const PAD_INDIFFERENT: ::base64::engine::GeneralPurposeConfig =
    ::base64::engine::GeneralPurposeConfig::new()
        .with_decode_padding_mode(::base64::engine::DecodePaddingMode::Indifferent);

impl<'de, T, ALPHABET, FORMAT> DeserializeAs<'de, T> for Base64<ALPHABET, FORMAT>
where
    T: TryFrom<Vec<u8>>,
    ALPHABET: Alphabet,
    FORMAT: formats::Format,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper<T, ALPHABET>(PhantomData<(T, ALPHABET)>);

        impl<T, ALPHABET> Visitor<'_> for Helper<T, ALPHABET>
        where
            T: TryFrom<Vec<u8>>,
            ALPHABET: Alphabet,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a base64 encoded string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                use ::base64::Engine as _;

                let bytes =
                    ::base64::engine::GeneralPurpose::new(&ALPHABET::charset(), PAD_INDIFFERENT)
                        .decode(value)
                        .map_err(DeError::custom)?;

                let length = bytes.len();
                bytes.try_into().map_err(|_e: T::Error| {
                    DeError::custom(format_args!(
                        "Can't convert a Byte Vector of length {length} to the output type."
                    ))
                })
            }
        }

        deserializer.deserialize_str(Helper::<T, ALPHABET>(PhantomData))
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Standard {}
    impl Sealed for super::UrlSafe {}
    impl Sealed for super::Crypt {}
    impl Sealed for super::Bcrypt {}
    impl Sealed for super::ImapMutf7 {}
    impl Sealed for super::BinHex {}
}

/// A base64 alphabet
pub trait Alphabet: sealed::Sealed {
    /// Return a specific alphabet.
    fn charset() -> ::base64::alphabet::Alphabet;
}
/// The standard character set (uses `+` and `/`).
///
/// See [RFC 3548](https://tools.ietf.org/html/rfc3548#section-3).
pub struct Standard;
impl Alphabet for Standard {
    fn charset() -> ::base64::alphabet::Alphabet {
        ::base64::alphabet::STANDARD
    }
}

/// The URL safe character set (uses `-` and `_`).
///
/// See [RFC 3548](https://tools.ietf.org/html/rfc3548#section-3).
pub struct UrlSafe;
impl Alphabet for UrlSafe {
    fn charset() -> ::base64::alphabet::Alphabet {
        ::base64::alphabet::URL_SAFE
    }
}

/// The `crypt(3)` character set (uses `./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz`).
///
/// Not standardized, but folk wisdom on the net asserts that this alphabet is what crypt uses.
pub struct Crypt;
impl Alphabet for Crypt {
    fn charset() -> ::base64::alphabet::Alphabet {
        ::base64::alphabet::CRYPT
    }
}

/// The bcrypt character set (uses `./ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789`).
pub struct Bcrypt;
impl Alphabet for Bcrypt {
    fn charset() -> ::base64::alphabet::Alphabet {
        ::base64::alphabet::BCRYPT
    }
}

/// The character set used in IMAP-modified UTF-7 (uses `+` and `,`).
///
/// See [RFC 3501](https://tools.ietf.org/html/rfc3501#section-5.1.3).
pub struct ImapMutf7;
impl Alphabet for ImapMutf7 {
    fn charset() -> ::base64::alphabet::Alphabet {
        ::base64::alphabet::IMAP_MUTF7
    }
}

/// The character set used in `BinHex` 4.0 files.
///
/// See [BinHex 4.0 Definition](http://files.stairways.com/other/binhex-40-specs-info.txt).
pub struct BinHex;
impl Alphabet for BinHex {
    fn charset() -> ::base64::alphabet::Alphabet {
        ::base64::alphabet::BIN_HEX
    }
}
