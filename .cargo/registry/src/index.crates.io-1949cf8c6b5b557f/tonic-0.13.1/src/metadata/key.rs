use bytes::Bytes;
use http::header::HeaderName;
use std::borrow::Borrow;
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;

use super::encoding::{Ascii, Binary, ValueEncoding};

/// Represents a custom metadata field name.
///
/// `MetadataKey` is used as the [`MetadataMap`] key.
///
/// [`HeaderMap`]: struct.HeaderMap.html
/// [`MetadataMap`]: struct.MetadataMap.html
#[derive(Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct MetadataKey<VE: ValueEncoding> {
    // Note: There are unsafe transmutes that assume that the memory layout
    // of MetadataValue is identical to HeaderName
    pub(crate) inner: http::header::HeaderName,
    phantom: PhantomData<VE>,
}

/// A possible error when converting a `MetadataKey` from another type.
#[derive(Debug, Default)]
pub struct InvalidMetadataKey {
    _priv: (),
}

/// An ascii metadata key.
pub type AsciiMetadataKey = MetadataKey<Ascii>;
/// A binary metadata key.
pub type BinaryMetadataKey = MetadataKey<Binary>;

impl<VE: ValueEncoding> MetadataKey<VE> {
    /// Converts a slice of bytes to a `MetadataKey`.
    ///
    /// This function normalizes the input.
    pub fn from_bytes(src: &[u8]) -> Result<Self, InvalidMetadataKey> {
        match HeaderName::from_bytes(src) {
            Ok(name) => {
                if !VE::is_valid_key(name.as_str()) {
                    return Err(InvalidMetadataKey::new());
                }

                Ok(MetadataKey {
                    inner: name,
                    phantom: PhantomData,
                })
            }
            Err(_) => Err(InvalidMetadataKey::new()),
        }
    }

    /// Converts a static string to a `MetadataKey`.
    ///
    /// This function panics when the static string is a invalid metadata key.
    ///
    /// This function requires the static string to only contain lowercase
    /// characters, numerals and symbols, as per the HTTP/2.0 specification
    /// and header names internal representation within this library.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// // Parsing a metadata key
    /// let CUSTOM_KEY: &'static str = "custom-key";
    ///
    /// let a = AsciiMetadataKey::from_bytes(b"custom-key").unwrap();
    /// let b = AsciiMetadataKey::from_static(CUSTOM_KEY);
    /// assert_eq!(a, b);
    /// ```
    ///
    /// ```should_panic
    /// # use tonic::metadata::*;
    /// // Parsing a metadata key that contains invalid symbols(s):
    /// AsciiMetadataKey::from_static("content{}{}length"); // This line panics!
    /// ```
    ///
    /// ```should_panic
    /// # use tonic::metadata::*;
    /// // Parsing a metadata key that contains invalid uppercase characters.
    /// let a = AsciiMetadataKey::from_static("foobar");
    /// let b = AsciiMetadataKey::from_static("FOOBAR"); // This line panics!
    /// ```
    ///
    /// ```should_panic
    /// # use tonic::metadata::*;
    /// // Parsing a -bin metadata key as an Ascii key.
    /// let b = AsciiMetadataKey::from_static("hello-bin"); // This line panics!
    /// ```
    ///
    /// ```should_panic
    /// # use tonic::metadata::*;
    /// // Parsing a non-bin metadata key as an Binary key.
    /// let b = BinaryMetadataKey::from_static("hello"); // This line panics!
    /// ```
    pub fn from_static(src: &'static str) -> Self {
        let name = HeaderName::from_static(src);
        if !VE::is_valid_key(name.as_str()) {
            panic!("invalid metadata key")
        }

        MetadataKey {
            inner: name,
            phantom: PhantomData,
        }
    }

    /// Returns a `str` representation of the metadata key.
    ///
    /// The returned string will always be lower case.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Converts a HeaderName reference to a MetadataKey. This method assumes
    /// that the caller has made sure that the header name has the correct
    /// "-bin" or non-"-bin" suffix, it does not validate its input.
    #[inline]
    pub(crate) fn unchecked_from_header_name_ref(header_name: &HeaderName) -> &Self {
        unsafe { &*(header_name as *const HeaderName as *const Self) }
    }

    /// Converts a HeaderName reference to a MetadataKey. This method assumes
    /// that the caller has made sure that the header name has the correct
    /// "-bin" or non-"-bin" suffix, it does not validate its input.
    #[inline]
    pub(crate) fn unchecked_from_header_name(name: HeaderName) -> Self {
        MetadataKey {
            inner: name,
            phantom: PhantomData,
        }
    }
}

impl<VE: ValueEncoding> FromStr for MetadataKey<VE> {
    type Err = InvalidMetadataKey;

    fn from_str(s: &str) -> Result<Self, InvalidMetadataKey> {
        MetadataKey::from_bytes(s.as_bytes()).map_err(|_| InvalidMetadataKey::new())
    }
}

impl<VE: ValueEncoding> AsRef<str> for MetadataKey<VE> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<VE: ValueEncoding> AsRef<[u8]> for MetadataKey<VE> {
    fn as_ref(&self) -> &[u8] {
        self.as_str().as_bytes()
    }
}

impl<VE: ValueEncoding> Borrow<str> for MetadataKey<VE> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl<VE: ValueEncoding> fmt::Debug for MetadataKey<VE> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), fmt)
    }
}

impl<VE: ValueEncoding> fmt::Display for MetadataKey<VE> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), fmt)
    }
}

impl InvalidMetadataKey {
    #[doc(hidden)]
    pub fn new() -> InvalidMetadataKey {
        Self::default()
    }
}

impl<'a, VE: ValueEncoding> From<&'a MetadataKey<VE>> for MetadataKey<VE> {
    fn from(src: &'a MetadataKey<VE>) -> MetadataKey<VE> {
        src.clone()
    }
}

impl<VE: ValueEncoding> From<MetadataKey<VE>> for Bytes {
    #[inline]
    fn from(name: MetadataKey<VE>) -> Bytes {
        Bytes::copy_from_slice(name.inner.as_ref())
    }
}

impl<'a, VE: ValueEncoding> PartialEq<&'a MetadataKey<VE>> for MetadataKey<VE> {
    #[inline]
    fn eq(&self, other: &&'a MetadataKey<VE>) -> bool {
        *self == **other
    }
}

impl<VE: ValueEncoding> PartialEq<MetadataKey<VE>> for &MetadataKey<VE> {
    #[inline]
    fn eq(&self, other: &MetadataKey<VE>) -> bool {
        *other == *self
    }
}

impl<VE: ValueEncoding> PartialEq<str> for MetadataKey<VE> {
    /// Performs a case-insensitive comparison of the string against the header
    /// name
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let content_length = AsciiMetadataKey::from_static("content-length");
    ///
    /// assert_eq!(content_length, "content-length");
    /// assert_eq!(content_length, "Content-Length");
    /// assert_ne!(content_length, "content length");
    /// ```
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.inner.eq(other)
    }
}

impl<VE: ValueEncoding> PartialEq<MetadataKey<VE>> for str {
    /// Performs a case-insensitive comparison of the string against the header
    /// name
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let content_length = AsciiMetadataKey::from_static("content-length");
    ///
    /// assert_eq!(content_length, "content-length");
    /// assert_eq!(content_length, "Content-Length");
    /// assert_ne!(content_length, "content length");
    /// ```
    #[inline]
    fn eq(&self, other: &MetadataKey<VE>) -> bool {
        other.inner == *self
    }
}

impl<'a, VE: ValueEncoding> PartialEq<&'a str> for MetadataKey<VE> {
    /// Performs a case-insensitive comparison of the string against the header
    /// name
    #[inline]
    fn eq(&self, other: &&'a str) -> bool {
        *self == **other
    }
}

impl<VE: ValueEncoding> PartialEq<MetadataKey<VE>> for &str {
    /// Performs a case-insensitive comparison of the string against the header
    /// name
    #[inline]
    fn eq(&self, other: &MetadataKey<VE>) -> bool {
        *other == *self
    }
}

impl fmt::Display for InvalidMetadataKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid gRPC metadata key name")
    }
}

impl Error for InvalidMetadataKey {}

#[cfg(test)]
mod tests {
    use super::{AsciiMetadataKey, BinaryMetadataKey};

    #[test]
    fn test_from_bytes_binary() {
        assert!(BinaryMetadataKey::from_bytes(b"").is_err());
        assert!(BinaryMetadataKey::from_bytes(b"\xFF").is_err());
        assert!(BinaryMetadataKey::from_bytes(b"abc").is_err());
        assert_eq!(
            BinaryMetadataKey::from_bytes(b"abc-bin").unwrap().as_str(),
            "abc-bin"
        );
    }

    #[test]
    fn test_from_bytes_ascii() {
        assert!(AsciiMetadataKey::from_bytes(b"").is_err());
        assert!(AsciiMetadataKey::from_bytes(b"\xFF").is_err());
        assert_eq!(
            AsciiMetadataKey::from_bytes(b"abc").unwrap().as_str(),
            "abc"
        );
        assert!(AsciiMetadataKey::from_bytes(b"abc-bin").is_err());
    }
}
