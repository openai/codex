use super::encoding::{
    Ascii, Binary, InvalidMetadataValue, InvalidMetadataValueBytes, ValueEncoding,
};
use super::key::MetadataKey;

use bytes::Bytes;
use http::header::HeaderValue;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;
use std::{cmp, fmt};

/// Represents a custom metadata field value.
///
/// `MetadataValue` is used as the [`MetadataMap`] value.
///
/// [`HeaderMap`]: struct.HeaderMap.html
/// [`MetadataMap`]: struct.MetadataMap.html
#[derive(Clone)]
#[repr(transparent)]
pub struct MetadataValue<VE: ValueEncoding> {
    // Note: There are unsafe transmutes that assume that the memory layout
    // of MetadataValue is identical to HeaderValue
    pub(crate) inner: HeaderValue,
    phantom: PhantomData<VE>,
}

/// A possible error when converting a `MetadataValue` to a string representation.
///
/// Metadata field values may contain opaque bytes, in which case it is not
/// possible to represent the value as a string.
#[derive(Debug)]
pub struct ToStrError {
    _priv: (),
}

/// An ascii metadata value.
pub type AsciiMetadataValue = MetadataValue<Ascii>;
/// A binary metadata value.
pub type BinaryMetadataValue = MetadataValue<Binary>;

impl<VE: ValueEncoding> MetadataValue<VE> {
    /// Convert a static string to a `MetadataValue`.
    ///
    /// This function will not perform any copying, however the string is
    /// checked to ensure that no invalid characters are present.
    ///
    /// For Ascii values, only visible ASCII characters (32-127) are permitted.
    /// For Binary values, the string must be valid base64.
    ///
    /// # Panics
    ///
    /// This function panics if the argument contains invalid metadata value
    /// characters.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let val = AsciiMetadataValue::from_static("hello");
    /// assert_eq!(val, "hello");
    /// ```
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let val = BinaryMetadataValue::from_static("SGVsbG8hIQ==");
    /// assert_eq!(val, "Hello!!");
    /// ```
    #[inline]
    pub fn from_static(src: &'static str) -> Self {
        MetadataValue {
            inner: VE::from_static(src),
            phantom: PhantomData,
        }
    }

    /// Convert a `Bytes` directly into a `MetadataValue` without validating.
    /// For `MetadataValue<Binary>` the provided parameter must be base64
    /// encoded without padding bytes at the end.
    ///
    /// # Safety
    ///
    /// This function does NOT validate that illegal bytes are not contained
    /// within the buffer.
    #[inline]
    pub unsafe fn from_shared_unchecked(src: Bytes) -> Self {
        MetadataValue {
            inner: HeaderValue::from_maybe_shared_unchecked(src),
            phantom: PhantomData,
        }
    }

    /// Returns true if the `MetadataValue` has a length of zero bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let val = AsciiMetadataValue::from_static("");
    /// assert!(val.is_empty());
    ///
    /// let val = AsciiMetadataValue::from_static("hello");
    /// assert!(!val.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        VE::is_empty(self.inner.as_bytes())
    }

    /// Converts a `MetadataValue` to a Bytes buffer. This method cannot
    /// fail for Ascii values. For Ascii values, `as_bytes` is more convenient
    /// to use.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let val = AsciiMetadataValue::from_static("hello");
    /// assert_eq!(val.to_bytes().unwrap().as_ref(), b"hello");
    /// ```
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let val = BinaryMetadataValue::from_bytes(b"hello");
    /// assert_eq!(val.to_bytes().unwrap().as_ref(), b"hello");
    /// ```
    #[inline]
    pub fn to_bytes(&self) -> Result<Bytes, InvalidMetadataValueBytes> {
        VE::decode(self.inner.as_bytes())
    }

    /// Mark that the metadata value represents sensitive information.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut val = AsciiMetadataValue::from_static("my secret");
    ///
    /// val.set_sensitive(true);
    /// assert!(val.is_sensitive());
    ///
    /// val.set_sensitive(false);
    /// assert!(!val.is_sensitive());
    /// ```
    #[inline]
    pub fn set_sensitive(&mut self, val: bool) {
        self.inner.set_sensitive(val);
    }

    /// Returns `true` if the value represents sensitive data.
    ///
    /// Sensitive data could represent passwords or other data that should not
    /// be stored on disk or in memory. This setting can be used by components
    /// like caches to avoid storing the value. HPACK encoders must set the
    /// metadata field to never index when `is_sensitive` returns true.
    ///
    /// Note that sensitivity is not factored into equality or ordering.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let mut val = AsciiMetadataValue::from_static("my secret");
    ///
    /// val.set_sensitive(true);
    /// assert!(val.is_sensitive());
    ///
    /// val.set_sensitive(false);
    /// assert!(!val.is_sensitive());
    /// ```
    #[inline]
    pub fn is_sensitive(&self) -> bool {
        self.inner.is_sensitive()
    }

    /// Converts a `MetadataValue` to a byte slice. For Binary values, the
    /// return value is base64 encoded.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let val = AsciiMetadataValue::from_static("hello");
    /// assert_eq!(val.as_encoded_bytes(), b"hello");
    /// ```
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let val = BinaryMetadataValue::from_bytes(b"Hello!");
    /// assert_eq!(val.as_encoded_bytes(), b"SGVsbG8h");
    /// ```
    #[inline]
    pub fn as_encoded_bytes(&self) -> &[u8] {
        self.inner.as_bytes()
    }

    /// Converts a HeaderValue to a MetadataValue. This method assumes that the
    /// caller has made sure that the value is of the correct Ascii or Binary
    /// value encoding.
    #[inline]
    pub(crate) fn unchecked_from_header_value(value: HeaderValue) -> Self {
        MetadataValue {
            inner: value,
            phantom: PhantomData,
        }
    }

    /// Converts a HeaderValue reference to a MetadataValue. This method assumes
    /// that the caller has made sure that the value is of the correct Ascii or
    /// Binary value encoding.
    #[inline]
    pub(crate) fn unchecked_from_header_value_ref(header_value: &HeaderValue) -> &Self {
        unsafe { &*(header_value as *const HeaderValue as *const Self) }
    }

    /// Converts a HeaderValue reference to a MetadataValue. This method assumes
    /// that the caller has made sure that the value is of the correct Ascii or
    /// Binary value encoding.
    #[inline]
    pub(crate) fn unchecked_from_mut_header_value_ref(header_value: &mut HeaderValue) -> &mut Self {
        unsafe { &mut *(header_value as *mut HeaderValue as *mut Self) }
    }
}

/// Attempt to convert a byte slice to a `MetadataValue`.
///
/// For Ascii metadata values, If the argument contains invalid metadata
/// value bytes, an error is returned. Only byte values between 32 and 255
/// (inclusive) are permitted, excluding byte 127 (DEL).
///
/// For Binary metadata values this method cannot fail. See also the Binary
/// only version of this method `from_bytes`.
///
/// # Examples
///
/// ```
/// # use tonic::metadata::*;
/// let val = AsciiMetadataValue::try_from(b"hello\xfa").unwrap();
/// assert_eq!(val, &b"hello\xfa"[..]);
/// ```
///
/// An invalid value
///
/// ```
/// # use tonic::metadata::*;
/// let val = AsciiMetadataValue::try_from(b"\n");
/// assert!(val.is_err());
/// ```
impl<VE: ValueEncoding> TryFrom<&[u8]> for MetadataValue<VE> {
    type Error = InvalidMetadataValueBytes;

    #[inline]
    fn try_from(src: &[u8]) -> Result<Self, Self::Error> {
        VE::from_bytes(src).map(|value| MetadataValue {
            inner: value,
            phantom: PhantomData,
        })
    }
}

/// Attempt to convert a byte slice to a `MetadataValue`.
///
/// For Ascii metadata values, If the argument contains invalid metadata
/// value bytes, an error is returned. Only byte values between 32 and 255
/// (inclusive) are permitted, excluding byte 127 (DEL).
///
/// For Binary metadata values this method cannot fail. See also the Binary
/// only version of this method `from_bytes`.
///
/// # Examples
///
/// ```
/// # use tonic::metadata::*;
/// let val = AsciiMetadataValue::try_from(b"hello\xfa").unwrap();
/// assert_eq!(val, &b"hello\xfa"[..]);
/// ```
///
/// An invalid value
///
/// ```
/// # use tonic::metadata::*;
/// let val = AsciiMetadataValue::try_from(b"\n");
/// assert!(val.is_err());
/// ```
impl<VE: ValueEncoding, const N: usize> TryFrom<&[u8; N]> for MetadataValue<VE> {
    type Error = InvalidMetadataValueBytes;

    #[inline]
    fn try_from(src: &[u8; N]) -> Result<Self, Self::Error> {
        Self::try_from(src.as_ref())
    }
}

/// Attempt to convert a `Bytes` buffer to a `MetadataValue`.
///
/// For `MetadataValue<Ascii>`, if the argument contains invalid metadata
/// value bytes, an error is returned. Only byte values between 32 and 255
/// (inclusive) are permitted, excluding byte 127 (DEL).
///
/// For `MetadataValue<Binary>`, if the argument is not valid base64, an
/// error is returned. In use cases where the input is not base64 encoded,
/// use `from_bytes`; if the value has to be encoded it's not possible to
/// share the memory anyways.
impl<VE: ValueEncoding> TryFrom<Bytes> for MetadataValue<VE> {
    type Error = InvalidMetadataValueBytes;

    #[inline]
    fn try_from(src: Bytes) -> Result<Self, Self::Error> {
        VE::from_shared(src).map(|value| MetadataValue {
            inner: value,
            phantom: PhantomData,
        })
    }
}

/// Attempt to convert a Vec of bytes to a `MetadataValue`.
///
/// For `MetadataValue<Ascii>`, if the argument contains invalid metadata
/// value bytes, an error is returned. Only byte values between 32 and 255
/// (inclusive) are permitted, excluding byte 127 (DEL).
///
/// For `MetadataValue<Binary>`, if the argument is not valid base64, an
/// error is returned. In use cases where the input is not base64 encoded,
/// use `from_bytes`; if the value has to be encoded it's not possible to
/// share the memory anyways.
impl<VE: ValueEncoding> TryFrom<Vec<u8>> for MetadataValue<VE> {
    type Error = InvalidMetadataValueBytes;

    #[inline]
    fn try_from(src: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from(src.as_slice())
    }
}

/// Attempt to convert a string to a `MetadataValue<Ascii>`.
///
/// If the argument contains invalid metadata value characters, an error is
/// returned. Only visible ASCII characters (32-127) are permitted. Use
/// `from_bytes` to create a `MetadataValue` that includes opaque octets
/// (128-255).
impl<'a> TryFrom<&'a str> for MetadataValue<Ascii> {
    type Error = InvalidMetadataValue;

    #[inline]
    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

/// Attempt to convert a string to a `MetadataValue<Ascii>`.
///
/// If the argument contains invalid metadata value characters, an error is
/// returned. Only visible ASCII characters (32-127) are permitted. Use
/// `from_bytes` to create a `MetadataValue` that includes opaque octets
/// (128-255).
impl<'a> TryFrom<&'a String> for MetadataValue<Ascii> {
    type Error = InvalidMetadataValue;

    #[inline]
    fn try_from(s: &'a String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

/// Attempt to convert a string to a `MetadataValue<Ascii>`.
///
/// If the argument contains invalid metadata value characters, an error is
/// returned. Only visible ASCII characters (32-127) are permitted. Use
/// `from_bytes` to create a `MetadataValue` that includes opaque octets
/// (128-255).
impl TryFrom<String> for MetadataValue<Ascii> {
    type Error = InvalidMetadataValue;

    #[inline]
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl MetadataValue<Ascii> {
    /// Converts a MetadataKey into a `MetadataValue<Ascii>`.
    ///
    /// Since every valid MetadataKey is a valid MetadataValue this is done
    /// infallibly.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let val = AsciiMetadataValue::from_key::<Ascii>("accept".parse().unwrap());
    /// assert_eq!(val, AsciiMetadataValue::try_from(b"accept").unwrap());
    /// ```
    #[inline]
    pub fn from_key<KeyVE: ValueEncoding>(key: MetadataKey<KeyVE>) -> Self {
        key.into()
    }

    /// Returns the length of `self`, in bytes.
    ///
    /// This method is not available for `MetadataValue<Binary>` because that
    /// cannot be implemented in constant time, which most people would probably
    /// expect. To get the length of `MetadataValue<Binary>`, convert it to a
    /// Bytes value and measure its length.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let val = AsciiMetadataValue::from_static("hello");
    /// assert_eq!(val.len(), 5);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Yields a `&str` slice if the `MetadataValue` only contains visible ASCII
    /// chars.
    ///
    /// This function will perform a scan of the metadata value, checking all the
    /// characters.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let val = AsciiMetadataValue::from_static("hello");
    /// assert_eq!(val.to_str().unwrap(), "hello");
    /// ```
    pub fn to_str(&self) -> Result<&str, ToStrError> {
        self.inner.to_str().map_err(|_| ToStrError::new())
    }

    /// Converts a `MetadataValue` to a byte slice. For Binary values, use
    /// `to_bytes`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let val = AsciiMetadataValue::from_static("hello");
    /// assert_eq!(val.as_bytes(), b"hello");
    /// ```
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.inner.as_bytes()
    }
}

impl MetadataValue<Binary> {
    /// Convert a byte slice to a `MetadataValue<Binary>`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tonic::metadata::*;
    /// let val = BinaryMetadataValue::from_bytes(b"hello\xfa");
    /// assert_eq!(val, &b"hello\xfa"[..]);
    /// ```
    #[inline]
    pub fn from_bytes(src: &[u8]) -> Self {
        // Only the Ascii version of try_from can fail.
        Self::try_from(src).unwrap()
    }
}

impl<VE: ValueEncoding> AsRef<[u8]> for MetadataValue<VE> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

impl<VE: ValueEncoding> fmt::Debug for MetadataValue<VE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        VE::fmt(&self.inner, f)
    }
}

impl<KeyVE: ValueEncoding> From<MetadataKey<KeyVE>> for MetadataValue<Ascii> {
    #[inline]
    fn from(h: MetadataKey<KeyVE>) -> MetadataValue<Ascii> {
        MetadataValue {
            inner: h.inner.into(),
            phantom: PhantomData,
        }
    }
}

macro_rules! from_integers {
    ($($name:ident: $t:ident => $max_len:expr),*) => {$(
        impl From<$t> for MetadataValue<Ascii> {
            fn from(num: $t) -> MetadataValue<Ascii> {
                MetadataValue {
                    inner: HeaderValue::from(num),
                    phantom: PhantomData,
                }
            }
        }

        #[test]
        fn $name() {
            let n: $t = 55;
            let val = AsciiMetadataValue::from(n);
            assert_eq!(val, &n.to_string());

            let n = $t::MAX;
            let val = AsciiMetadataValue::from(n);
            assert_eq!(val, &n.to_string());
        }
    )*};
}

from_integers! {
    // integer type => maximum decimal length

    // u8 purposely left off... AsciiMetadataValue::from(b'3') could be confusing
    from_u16: u16 => 5,
    from_i16: i16 => 6,
    from_u32: u32 => 10,
    from_i32: i32 => 11,
    from_u64: u64 => 20,
    from_i64: i64 => 20
}

#[cfg(target_pointer_width = "16")]
from_integers! {
    from_usize: usize => 5,
    from_isize: isize => 6
}

#[cfg(target_pointer_width = "32")]
from_integers! {
    from_usize: usize => 10,
    from_isize: isize => 11
}

#[cfg(target_pointer_width = "64")]
from_integers! {
    from_usize: usize => 20,
    from_isize: isize => 20
}

#[cfg(test)]
mod from_metadata_value_tests {
    use super::*;
    use crate::metadata::map::MetadataMap;

    #[test]
    fn it_can_insert_metadata_key_as_metadata_value() {
        let mut map = MetadataMap::new();
        map.insert(
            "accept",
            MetadataKey::<Ascii>::from_bytes(b"hello-world")
                .unwrap()
                .into(),
        );

        assert_eq!(
            map.get("accept").unwrap(),
            AsciiMetadataValue::try_from(b"hello-world").unwrap()
        );
    }
}

impl FromStr for MetadataValue<Ascii> {
    type Err = InvalidMetadataValue;

    #[inline]
    fn from_str(s: &str) -> Result<MetadataValue<Ascii>, Self::Err> {
        HeaderValue::from_str(s)
            .map(|value| MetadataValue {
                inner: value,
                phantom: PhantomData,
            })
            .map_err(|_| InvalidMetadataValue::new())
    }
}

impl<VE: ValueEncoding> From<MetadataValue<VE>> for Bytes {
    #[inline]
    fn from(value: MetadataValue<VE>) -> Bytes {
        Bytes::copy_from_slice(value.inner.as_bytes())
    }
}

impl<'a, VE: ValueEncoding> From<&'a MetadataValue<VE>> for MetadataValue<VE> {
    #[inline]
    fn from(t: &'a MetadataValue<VE>) -> Self {
        t.clone()
    }
}

// ===== ToStrError =====

impl ToStrError {
    pub(crate) fn new() -> Self {
        ToStrError { _priv: () }
    }
}

impl fmt::Display for ToStrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failed to convert metadata to a str")
    }
}

impl Error for ToStrError {}

impl Hash for MetadataValue<Ascii> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl Hash for MetadataValue<Binary> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.to_bytes() {
            Ok(b) => b.hash(state),
            Err(e) => e.hash(state),
        }
    }
}

// ===== PartialEq / PartialOrd =====

impl<VE: ValueEncoding> PartialEq for MetadataValue<VE> {
    #[inline]
    fn eq(&self, other: &MetadataValue<VE>) -> bool {
        // Note: Different binary strings that after base64 decoding
        // will count as the same value for Binary values. Also,
        // different invalid base64 values count as equal for Binary
        // values.
        VE::values_equal(&self.inner, &other.inner)
    }
}

impl<VE: ValueEncoding> Eq for MetadataValue<VE> {}

impl<VE: ValueEncoding> PartialOrd for MetadataValue<VE> {
    #[inline]
    fn partial_cmp(&self, other: &MetadataValue<VE>) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<VE: ValueEncoding> Ord for MetadataValue<VE> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl<VE: ValueEncoding> PartialEq<str> for MetadataValue<VE> {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        VE::equals(&self.inner, other.as_bytes())
    }
}

impl<VE: ValueEncoding> PartialEq<[u8]> for MetadataValue<VE> {
    #[inline]
    fn eq(&self, other: &[u8]) -> bool {
        VE::equals(&self.inner, other)
    }
}

impl<VE: ValueEncoding> PartialOrd<str> for MetadataValue<VE> {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        self.inner.partial_cmp(other.as_bytes())
    }
}

impl<VE: ValueEncoding> PartialOrd<[u8]> for MetadataValue<VE> {
    #[inline]
    fn partial_cmp(&self, other: &[u8]) -> Option<cmp::Ordering> {
        self.inner.partial_cmp(other)
    }
}

impl<VE: ValueEncoding> PartialEq<MetadataValue<VE>> for str {
    #[inline]
    fn eq(&self, other: &MetadataValue<VE>) -> bool {
        *other == *self
    }
}

impl<VE: ValueEncoding> PartialEq<MetadataValue<VE>> for [u8] {
    #[inline]
    fn eq(&self, other: &MetadataValue<VE>) -> bool {
        *other == *self
    }
}

impl<VE: ValueEncoding> PartialOrd<MetadataValue<VE>> for str {
    #[inline]
    fn partial_cmp(&self, other: &MetadataValue<VE>) -> Option<cmp::Ordering> {
        self.as_bytes().partial_cmp(other.inner.as_bytes())
    }
}

impl<VE: ValueEncoding> PartialOrd<MetadataValue<VE>> for [u8] {
    #[inline]
    fn partial_cmp(&self, other: &MetadataValue<VE>) -> Option<cmp::Ordering> {
        self.partial_cmp(other.inner.as_bytes())
    }
}

impl<VE: ValueEncoding> PartialEq<String> for MetadataValue<VE> {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        *self == other[..]
    }
}

impl<VE: ValueEncoding> PartialOrd<String> for MetadataValue<VE> {
    #[inline]
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        self.inner.partial_cmp(other.as_bytes())
    }
}

impl<VE: ValueEncoding> PartialEq<MetadataValue<VE>> for String {
    #[inline]
    fn eq(&self, other: &MetadataValue<VE>) -> bool {
        *other == *self
    }
}

impl<VE: ValueEncoding> PartialOrd<MetadataValue<VE>> for String {
    #[inline]
    fn partial_cmp(&self, other: &MetadataValue<VE>) -> Option<cmp::Ordering> {
        self.as_bytes().partial_cmp(other.inner.as_bytes())
    }
}

impl<VE: ValueEncoding> PartialEq<MetadataValue<VE>> for &MetadataValue<VE> {
    #[inline]
    fn eq(&self, other: &MetadataValue<VE>) -> bool {
        **self == *other
    }
}

impl<VE: ValueEncoding> PartialOrd<MetadataValue<VE>> for &MetadataValue<VE> {
    #[inline]
    fn partial_cmp(&self, other: &MetadataValue<VE>) -> Option<cmp::Ordering> {
        (**self).partial_cmp(other)
    }
}

impl<'a, VE: ValueEncoding, T: ?Sized> PartialEq<&'a T> for MetadataValue<VE>
where
    MetadataValue<VE>: PartialEq<T>,
{
    #[inline]
    fn eq(&self, other: &&'a T) -> bool {
        *self == **other
    }
}

impl<'a, VE: ValueEncoding, T: ?Sized> PartialOrd<&'a T> for MetadataValue<VE>
where
    MetadataValue<VE>: PartialOrd<T>,
{
    #[inline]
    fn partial_cmp(&self, other: &&'a T) -> Option<cmp::Ordering> {
        self.partial_cmp(*other)
    }
}

impl<VE: ValueEncoding> PartialEq<MetadataValue<VE>> for &str {
    #[inline]
    fn eq(&self, other: &MetadataValue<VE>) -> bool {
        *other == *self
    }
}

impl<VE: ValueEncoding> PartialOrd<MetadataValue<VE>> for &str {
    #[inline]
    fn partial_cmp(&self, other: &MetadataValue<VE>) -> Option<cmp::Ordering> {
        self.as_bytes().partial_cmp(other.inner.as_bytes())
    }
}

#[test]
fn test_debug() {
    let cases = &[
        ("hello", "\"hello\""),
        ("hello \"world\"", "\"hello \\\"world\\\"\""),
        ("\u{7FFF}hello", "\"\\xe7\\xbf\\xbfhello\""),
    ];

    for &(value, expected) in cases {
        let val = AsciiMetadataValue::try_from(value.as_bytes()).unwrap();
        let actual = format!("{val:?}");
        assert_eq!(expected, actual);
    }

    let mut sensitive = AsciiMetadataValue::from_static("password");
    sensitive.set_sensitive(true);
    assert_eq!("Sensitive", format!("{sensitive:?}"));
}

#[test]
fn test_is_empty() {
    fn from_str<VE: ValueEncoding>(s: &str) -> MetadataValue<VE> {
        MetadataValue::<VE>::unchecked_from_header_value(s.parse().unwrap())
    }

    assert!(from_str::<Ascii>("").is_empty());
    assert!(from_str::<Binary>("").is_empty());
    assert!(!from_str::<Ascii>("a").is_empty());
    assert!(!from_str::<Binary>("a").is_empty());
    assert!(!from_str::<Ascii>("=").is_empty());
    assert!(from_str::<Binary>("=").is_empty());
    assert!(!from_str::<Ascii>("===").is_empty());
    assert!(from_str::<Binary>("===").is_empty());
    assert!(!from_str::<Ascii>("=====").is_empty());
    assert!(from_str::<Binary>("=====").is_empty());
}

#[test]
fn test_from_shared_base64_encodes() {
    let value = BinaryMetadataValue::try_from(Bytes::from_static(b"Hello")).unwrap();
    assert_eq!(value.as_encoded_bytes(), b"SGVsbG8");
}

#[test]
fn test_value_eq_value() {
    type Bmv = BinaryMetadataValue;
    type Amv = AsciiMetadataValue;

    assert_eq!(Amv::from_static("abc"), Amv::from_static("abc"));
    assert_ne!(Amv::from_static("abc"), Amv::from_static("ABC"));

    assert_eq!(Bmv::from_bytes(b"abc"), Bmv::from_bytes(b"abc"));
    assert_ne!(Bmv::from_bytes(b"abc"), Bmv::from_bytes(b"ABC"));

    // Padding is ignored.
    assert_eq!(
        Bmv::from_static("SGVsbG8hIQ=="),
        Bmv::from_static("SGVsbG8hIQ")
    );
    // Invalid values are all just invalid from this point of view.
    unsafe {
        assert_eq!(
            Bmv::from_shared_unchecked(Bytes::from_static(b"..{}")),
            Bmv::from_shared_unchecked(Bytes::from_static(b"{}.."))
        );
    }
}

#[test]
fn test_value_eq_str() {
    type Bmv = BinaryMetadataValue;
    type Amv = AsciiMetadataValue;

    assert_eq!(Amv::from_static("abc"), "abc");
    assert_ne!(Amv::from_static("abc"), "ABC");
    assert_eq!("abc", Amv::from_static("abc"));
    assert_ne!("ABC", Amv::from_static("abc"));

    assert_eq!(Bmv::from_bytes(b"abc"), "abc");
    assert_ne!(Bmv::from_bytes(b"abc"), "ABC");
    assert_eq!("abc", Bmv::from_bytes(b"abc"));
    assert_ne!("ABC", Bmv::from_bytes(b"abc"));

    // Padding is ignored.
    assert_eq!(Bmv::from_static("SGVsbG8hIQ=="), "Hello!!");
    assert_eq!("Hello!!", Bmv::from_static("SGVsbG8hIQ=="));
}

#[test]
fn test_value_eq_bytes() {
    type Bmv = BinaryMetadataValue;
    type Amv = AsciiMetadataValue;

    assert_eq!(Amv::from_static("abc"), "abc".as_bytes());
    assert_ne!(Amv::from_static("abc"), "ABC".as_bytes());
    assert_eq!(*"abc".as_bytes(), Amv::from_static("abc"));
    assert_ne!(*"ABC".as_bytes(), Amv::from_static("abc"));

    assert_eq!(*"abc".as_bytes(), Bmv::from_bytes(b"abc"));
    assert_ne!(*"ABC".as_bytes(), Bmv::from_bytes(b"abc"));

    // Padding is ignored.
    assert_eq!(Bmv::from_static("SGVsbG8hIQ=="), "Hello!!".as_bytes());
    assert_eq!(*"Hello!!".as_bytes(), Bmv::from_static("SGVsbG8hIQ=="));
}

#[test]
fn test_ascii_value_hash() {
    use std::collections::hash_map::DefaultHasher;
    type Amv = AsciiMetadataValue;

    fn hash(value: Amv) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    let value1 = Amv::from_static("abc");
    let value2 = Amv::from_static("abc");
    assert_eq!(value1, value2);
    assert_eq!(hash(value1), hash(value2));

    let value1 = Amv::from_static("abc");
    let value2 = Amv::from_static("xyz");

    assert_ne!(value1, value2);
    assert_ne!(hash(value1), hash(value2));
}

#[test]
fn test_valid_binary_value_hash() {
    use std::collections::hash_map::DefaultHasher;
    type Bmv = BinaryMetadataValue;

    fn hash(value: Bmv) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    let value1 = Bmv::from_bytes(b"abc");
    let value2 = Bmv::from_bytes(b"abc");
    assert_eq!(value1, value2);
    assert_eq!(hash(value1), hash(value2));

    let value1 = Bmv::from_bytes(b"abc");
    let value2 = Bmv::from_bytes(b"xyz");
    assert_ne!(value1, value2);
    assert_ne!(hash(value1), hash(value2));
}

#[test]
fn test_invalid_binary_value_hash() {
    use std::collections::hash_map::DefaultHasher;
    type Bmv = BinaryMetadataValue;

    fn hash(value: Bmv) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    unsafe {
        let value1 = Bmv::from_shared_unchecked(Bytes::from_static(b"..{}"));
        let value2 = Bmv::from_shared_unchecked(Bytes::from_static(b"{}.."));
        assert_eq!(value1, value2);
        assert_eq!(hash(value1), hash(value2));
    }

    unsafe {
        let valid = Bmv::from_bytes(b"abc");
        let invalid = Bmv::from_shared_unchecked(Bytes::from_static(b"{}.."));
        assert_ne!(valid, invalid);
        assert_ne!(hash(valid), hash(invalid));
    }
}
