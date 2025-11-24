//! A module for wrappers that encode / decode data.

use std::borrow::Cow;
use std::str::Utf8Error;

#[cfg(feature = "encoding")]
use encoding_rs::{DecoderResult, Encoding, UTF_16BE, UTF_16LE, UTF_8};

/// Unicode "byte order mark" (\u{FEFF}) encoded as UTF-8.
/// See <https://unicode.org/faq/utf_bom.html#bom1>
pub(crate) const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
/// Unicode "byte order mark" (\u{FEFF}) encoded as UTF-16 with little-endian byte order.
/// See <https://unicode.org/faq/utf_bom.html#bom1>
#[cfg(feature = "encoding")]
pub(crate) const UTF16_LE_BOM: &[u8] = &[0xFF, 0xFE];
/// Unicode "byte order mark" (\u{FEFF}) encoded as UTF-16 with big-endian byte order.
/// See <https://unicode.org/faq/utf_bom.html#bom1>
#[cfg(feature = "encoding")]
pub(crate) const UTF16_BE_BOM: &[u8] = &[0xFE, 0xFF];

/// An error when decoding or encoding
///
/// If feature [`encoding`] is disabled, the [`EncodingError`] is always [`EncodingError::Utf8`]
///
/// [`encoding`]: ../index.html#encoding
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodingError {
    /// Input was not valid UTF-8
    Utf8(Utf8Error),
    /// Input did not adhere to the given encoding
    #[cfg(feature = "encoding")]
    Other(&'static Encoding),
}

impl From<Utf8Error> for EncodingError {
    #[inline]
    fn from(e: Utf8Error) -> Self {
        Self::Utf8(e)
    }
}

impl std::error::Error for EncodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Utf8(e) => Some(e),
            #[cfg(feature = "encoding")]
            Self::Other(_) => None,
        }
    }
}

impl std::fmt::Display for EncodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Utf8(e) => write!(f, "cannot decode input using UTF-8: {}", e),
            #[cfg(feature = "encoding")]
            Self::Other(encoding) => write!(f, "cannot decode input using {}", encoding.name()),
        }
    }
}

/// Decoder of byte slices into strings.
///
/// If feature [`encoding`] is enabled, this encoding taken from the `"encoding"`
/// XML declaration or assumes UTF-8, if XML has no <?xml ?> declaration, encoding
/// key is not defined or contains unknown encoding.
///
/// The library supports any UTF-8 compatible encodings that crate `encoding_rs`
/// is supported. [*UTF-16 and ISO-2022-JP are not supported at the present*][utf16].
///
/// If feature [`encoding`] is disabled, the decoder is always UTF-8 decoder:
/// any XML declarations are ignored.
///
/// [utf16]: https://github.com/tafia/quick-xml/issues/158
/// [`encoding`]: ../index.html#encoding
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Decoder {
    #[cfg(feature = "encoding")]
    pub(crate) encoding: &'static Encoding,
}

impl Decoder {
    pub(crate) fn utf8() -> Self {
        Decoder {
            #[cfg(feature = "encoding")]
            encoding: UTF_8,
        }
    }

    #[cfg(all(test, feature = "encoding", feature = "serialize"))]
    pub(crate) fn utf16() -> Self {
        Decoder { encoding: UTF_16LE }
    }
}

impl Decoder {
    /// Returns the `Reader`s encoding.
    ///
    /// This encoding will be used by [`decode`].
    ///
    /// [`decode`]: Self::decode
    #[cfg(feature = "encoding")]
    pub const fn encoding(&self) -> &'static Encoding {
        self.encoding
    }

    /// ## Without `encoding` feature
    ///
    /// Decodes an UTF-8 slice regardless of XML declaration and ignoring BOM
    /// if it is present in the `bytes`.
    ///
    /// ## With `encoding` feature
    ///
    /// Decodes specified bytes using encoding, declared in the XML, if it was
    /// declared there, or UTF-8 otherwise, and ignoring BOM if it is present
    /// in the `bytes`.
    ///
    /// ----
    /// Returns an error in case of malformed sequences in the `bytes`.
    pub fn decode<'b>(&self, bytes: &'b [u8]) -> Result<Cow<'b, str>, EncodingError> {
        #[cfg(not(feature = "encoding"))]
        let decoded = Ok(Cow::Borrowed(std::str::from_utf8(bytes)?));

        #[cfg(feature = "encoding")]
        let decoded = decode(bytes, self.encoding);

        decoded
    }

    /// Like [`decode`][Self::decode] but using a pre-allocated buffer.
    pub fn decode_into(&self, bytes: &[u8], buf: &mut String) -> Result<(), EncodingError> {
        #[cfg(not(feature = "encoding"))]
        buf.push_str(std::str::from_utf8(bytes)?);

        #[cfg(feature = "encoding")]
        decode_into(bytes, self.encoding, buf)?;

        Ok(())
    }

    /// Decodes the `Cow` buffer, preserves the lifetime
    pub(crate) fn decode_cow<'b>(
        &self,
        bytes: &Cow<'b, [u8]>,
    ) -> Result<Cow<'b, str>, EncodingError> {
        match bytes {
            Cow::Borrowed(bytes) => self.decode(bytes),
            // Convert to owned, because otherwise Cow will be bound with wrong lifetime
            Cow::Owned(bytes) => Ok(self.decode(bytes)?.into_owned().into()),
        }
    }
}

/// Decodes the provided bytes using the specified encoding.
///
/// Returns an error in case of malformed or non-representable sequences in the `bytes`.
#[cfg(feature = "encoding")]
pub fn decode<'b>(
    bytes: &'b [u8],
    encoding: &'static Encoding,
) -> Result<Cow<'b, str>, EncodingError> {
    encoding
        .decode_without_bom_handling_and_without_replacement(bytes)
        .ok_or(EncodingError::Other(encoding))
}

/// Like [`decode`] but using a pre-allocated buffer.
#[cfg(feature = "encoding")]
pub fn decode_into(
    bytes: &[u8],
    encoding: &'static Encoding,
    buf: &mut String,
) -> Result<(), EncodingError> {
    if encoding == UTF_8 {
        buf.push_str(std::str::from_utf8(bytes)?);
        return Ok(());
    }

    let mut decoder = encoding.new_decoder_without_bom_handling();
    buf.reserve(
        decoder
            .max_utf8_buffer_length_without_replacement(bytes.len())
            // SAFETY: None can be returned only if required size will overflow usize,
            // but in that case String::reserve also panics
            .unwrap(),
    );
    let (result, read) = decoder.decode_to_string_without_replacement(bytes, buf, true);
    match result {
        DecoderResult::InputEmpty => {
            debug_assert_eq!(read, bytes.len());
            Ok(())
        }
        DecoderResult::Malformed(_, _) => Err(EncodingError::Other(encoding)),
        // SAFETY: We allocate enough space above
        DecoderResult::OutputFull => unreachable!(),
    }
}

/// Automatic encoding detection of XML files based using the
/// [recommended algorithm](https://www.w3.org/TR/xml11/#sec-guessing).
///
/// If encoding is detected, `Some` is returned with an encoding and size of BOM
/// in bytes, if detection was performed using BOM, or zero, if detection was
/// performed without BOM.
///
/// IF encoding was not recognized, `None` is returned.
///
/// Because the [`encoding_rs`] crate supports only subset of those encodings, only
/// the supported subset are detected, which is UTF-8, UTF-16 BE and UTF-16 LE.
///
/// The algorithm suggests examine up to the first 4 bytes to determine encoding
/// according to the following table:
///
/// | Bytes       |Detected encoding
/// |-------------|------------------------------------------
/// | **BOM**
/// |`FE_FF_##_##`|UTF-16, big-endian
/// |`FF FE ## ##`|UTF-16, little-endian
/// |`EF BB BF`   |UTF-8
/// | **No BOM**
/// |`00 3C 00 3F`|UTF-16 BE or ISO-10646-UCS-2 BE or similar 16-bit BE (use declared encoding to find the exact one)
/// |`3C 00 3F 00`|UTF-16 LE or ISO-10646-UCS-2 LE or similar 16-bit LE (use declared encoding to find the exact one)
/// |`3C 3F 78 6D`|UTF-8, ISO 646, ASCII, some part of ISO 8859, Shift-JIS, EUC, or any other 7-bit, 8-bit, or mixed-width encoding which ensures that the characters of ASCII have their normal positions, width, and values; the actual encoding declaration must be read to detect which of these applies, but since all of these encodings use the same bit patterns for the relevant ASCII characters, the encoding declaration itself may be read reliably
#[cfg(feature = "encoding")]
pub fn detect_encoding(bytes: &[u8]) -> Option<(&'static Encoding, usize)> {
    match bytes {
        // with BOM
        _ if bytes.starts_with(UTF16_BE_BOM) => Some((UTF_16BE, 2)),
        _ if bytes.starts_with(UTF16_LE_BOM) => Some((UTF_16LE, 2)),
        _ if bytes.starts_with(UTF8_BOM) => Some((UTF_8, 3)),

        // without BOM
        _ if bytes.starts_with(&[0x00, b'<', 0x00, b'?']) => Some((UTF_16BE, 0)), // Some BE encoding, for example, UTF-16 or ISO-10646-UCS-2
        _ if bytes.starts_with(&[b'<', 0x00, b'?', 0x00]) => Some((UTF_16LE, 0)), // Some LE encoding, for example, UTF-16 or ISO-10646-UCS-2
        _ if bytes.starts_with(&[b'<', b'?', b'x', b'm']) => Some((UTF_8, 0)), // Some ASCII compatible

        _ => None,
    }
}
