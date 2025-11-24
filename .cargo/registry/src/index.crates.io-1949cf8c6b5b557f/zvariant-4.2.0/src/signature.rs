use core::{
    cmp::Ordering,
    fmt::{self, Debug, Display, Formatter},
    hash::{Hash, Hasher},
    str,
};
use serde::{
    de::{Deserialize, Deserializer},
    ser::{Serialize, Serializer},
};
use static_assertions::assert_impl_all;
use std::{
    borrow::Cow,
    ops::{Bound, RangeBounds},
    sync::Arc,
};

use crate::{serialized::Format, signature_parser::SignatureParser, Basic, Error, Result, Type};

// A data type similar to Cow and [`bytes::Bytes`] but unlike the former won't allow us to only keep
// the owned bytes in Arc and latter doesn't have a notion of borrowed data and would require API
// breakage.
//
// [`bytes::Bytes`]: https://docs.rs/bytes/0.5.6/bytes/struct.Bytes.html
#[derive(Debug, Clone)]
enum Bytes<'b> {
    Borrowed(&'b [u8]),
    Static(&'static [u8]),
    Owned(Arc<[u8]>),
}

impl<'b> Bytes<'b> {
    const fn borrowed<'s: 'b>(bytes: &'s [u8]) -> Self {
        Self::Borrowed(bytes)
    }

    fn owned(bytes: Vec<u8>) -> Self {
        Self::Owned(bytes.into())
    }

    /// This is faster than `Clone::clone` when `self` contains owned data.
    fn as_ref(&self) -> Bytes<'_> {
        match &self {
            Bytes::Static(s) => Bytes::Static(s),
            Bytes::Borrowed(s) => Bytes::Borrowed(s),
            Bytes::Owned(s) => Bytes::Borrowed(s),
        }
    }
}

impl std::ops::Deref for Bytes<'_> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Bytes::Borrowed(borrowed) => borrowed,
            Bytes::Static(borrowed) => borrowed,
            Bytes::Owned(owned) => owned,
        }
    }
}

impl Eq for Bytes<'_> {}

impl PartialEq for Bytes<'_> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl Ord for Bytes<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        (**self).cmp(&**other)
    }
}

impl PartialOrd for Bytes<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for Bytes<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state)
    }
}

/// String that [identifies] the type of an encoded value.
///
/// # Examples
///
/// ```
/// use zvariant::Signature;
///
/// // Valid signatures
/// let s = Signature::try_from("").unwrap();
/// assert_eq!(s, "");
/// # assert_eq!(s.n_complete_types(), Ok(0));
/// let s = Signature::try_from("y").unwrap();
/// assert_eq!(s, "y");
/// # assert_eq!(s.n_complete_types(), Ok(1));
/// let s = Signature::try_from("xs").unwrap();
/// assert_eq!(s, "xs");
/// # assert_eq!(s.n_complete_types(), Ok(2));
/// let s = Signature::try_from("(ysa{sd})").unwrap();
/// assert_eq!(s, "(ysa{sd})");
/// # assert_eq!(s.n_complete_types(), Ok(1));
/// let s = Signature::try_from("a{sd}").unwrap();
/// assert_eq!(s, "a{sd}");
/// # assert_eq!(s.n_complete_types(), Ok(1));
///
/// // Invalid signatures
/// Signature::try_from("z").unwrap_err();
/// Signature::try_from("(xs").unwrap_err();
/// Signature::try_from("xs)").unwrap_err();
/// Signature::try_from("s/").unwrap_err();
/// Signature::try_from("a").unwrap_err();
/// Signature::try_from("a{yz}").unwrap_err();
/// ```
///
/// This is implemented so that multiple instances can share the same underlying signature string.
/// Use [`slice`] method to create new signature that represents a portion of a signature
///
/// [identifies]: https://dbus.freedesktop.org/doc/dbus-specification.html#type-system
/// [`slice`]: #method.slice
#[derive(Hash, Clone, PartialOrd, Ord)]
pub struct Signature<'a> {
    bytes: Bytes<'a>,
    pos: usize,
    end: usize,
}

assert_impl_all!(Signature<'_>: Send, Sync, Unpin);

impl<'a> Signature<'a> {
    /// The signature as a string.
    pub fn as_str(&self) -> &str {
        // SAFETY: non-UTF8 characters in Signature are rejected by safe constructors
        unsafe { str::from_utf8_unchecked(self.as_bytes()) }
    }

    /// The signature bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[self.pos..self.end]
    }

    /// This is faster than `Clone::clone` when `self` contains owned data.
    pub fn as_ref(&self) -> Signature<'_> {
        Signature {
            bytes: self.bytes.as_ref(),
            pos: self.pos,
            end: self.end,
        }
    }

    /// Create a new Signature from given bytes.
    ///
    /// Since the passed bytes are not checked for correctness, it's provided for ease of
    /// `Type` implementations.
    ///
    /// # Safety
    ///
    /// This method is unsafe as it allows creating a `str` that is not valid UTF-8.
    pub unsafe fn from_bytes_unchecked<'s: 'a>(bytes: &'s [u8]) -> Self {
        Self {
            bytes: Bytes::borrowed(bytes),
            pos: 0,
            end: bytes.len(),
        }
    }

    /// Same as `from_bytes_unchecked`, except it takes a static reference.
    ///
    /// # Safety
    ///
    /// This method is unsafe as it allows creating a `str` that is not valid UTF-8.
    pub unsafe fn from_static_bytes_unchecked(bytes: &'static [u8]) -> Self {
        Self {
            bytes: Bytes::Static(bytes),
            pos: 0,
            end: bytes.len(),
        }
    }

    /// Same as `from_bytes_unchecked`, except it takes a string reference.
    pub const fn from_str_unchecked<'s: 'a>(signature: &'s str) -> Self {
        Self {
            bytes: Bytes::borrowed(signature.as_bytes()),
            pos: 0,
            end: signature.len(),
        }
    }

    /// Same as `from_str_unchecked`, except it takes a static string reference.
    pub const fn from_static_str_unchecked(signature: &'static str) -> Self {
        Self {
            bytes: Bytes::Static(signature.as_bytes()),
            pos: 0,
            end: signature.len(),
        }
    }

    /// Same as `from_str_unchecked`, except it takes an owned `String`.
    pub fn from_string_unchecked(signature: String) -> Self {
        let bytes = signature.into_bytes();
        let end = bytes.len();

        Self {
            bytes: Bytes::owned(bytes),
            pos: 0,
            end,
        }
    }

    /// Same as `from_static_str_unchecked`, except it checks validity of the signature.
    ///
    /// It's recommended to use this method instead of `TryFrom<&str>` implementation for
    /// `&'static str`. The former will ensure that [`Signature::to_owned`] and
    /// [`Signature::into_owned`] do not clone the underlying bytes.
    pub fn from_static_str(signature: &'static str) -> Result<Self> {
        let bytes = signature.as_bytes();
        SignatureParser::validate(bytes)?;

        Ok(Self {
            bytes: Bytes::Static(bytes),
            pos: 0,
            end: signature.len(),
        })
    }

    /// Same as `from_static_bytes_unchecked`, except it checks validity of the signature.
    ///
    /// It's recommended to use this method instead of the `TryFrom<&[u8]>` implementation for
    /// `&'static [u8]`. The former will ensure that [`Signature::to_owned`] and
    /// [`Signature::into_owned`] do not clone the underlying bytes.
    pub fn from_static_bytes(bytes: &'static [u8]) -> Result<Self> {
        SignatureParser::validate(bytes)?;

        Ok(Self {
            bytes: Bytes::Static(bytes),
            pos: 0,
            end: bytes.len(),
        })
    }

    /// the signature's length.
    pub fn len(&self) -> usize {
        self.end - self.pos
    }

    /// if the signature is empty.
    pub fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }

    /// Creates an owned clone of `self`.
    pub fn to_owned(&self) -> Signature<'static> {
        match &self.bytes {
            Bytes::Borrowed(_) => {
                let bytes = Bytes::owned(self.as_bytes().to_vec());
                let pos = 0;
                let end = bytes.len();

                Signature { bytes, pos, end }
            }
            Bytes::Static(b) => Signature {
                bytes: Bytes::Static(b),
                pos: self.pos,
                end: self.end,
            },
            Bytes::Owned(owned) => Signature {
                bytes: Bytes::Owned(owned.clone()),
                pos: self.pos,
                end: self.end,
            },
        }
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> Signature<'static> {
        self.to_owned()
    }

    /// Returns a slice of `self` for the provided range.
    ///
    /// # Panics
    ///
    /// Requires that begin <= end and end <= self.len(), otherwise slicing will panic.
    #[must_use]
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Self {
        let len = self.len();

        let pos = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => len,
        };

        assert!(
            pos <= end,
            "range start must not be greater than end: {:?} > {:?}",
            pos,
            end,
        );
        assert!(end <= len, "range end out of bounds: {:?} > {:?}", end, len,);

        if end == pos {
            return Self::from_str_unchecked("");
        }

        let mut clone = self.clone();
        clone.pos += pos;
        clone.end = self.pos + end;

        clone
    }

    /// The number of complete types for the signature.
    ///
    /// # Errors
    ///
    /// If the signature is invalid, returns the first error.
    pub fn n_complete_types(&self) -> Result<usize> {
        let mut count = 0;
        // SAFETY: the parser is only used to do counting
        for s in unsafe { SignatureParser::from_bytes_unchecked(self.as_bytes())? } {
            s?;
            count += 1;
        }
        Ok(count)
    }
}

impl<'a> Debug for Signature<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Signature").field(&self.as_str()).finish()
    }
}

impl<'a> Basic for Signature<'a> {
    const SIGNATURE_CHAR: char = 'g';
    const SIGNATURE_STR: &'static str = "g";

    fn alignment(format: Format) -> usize {
        match format {
            Format::DBus => 1,
            #[cfg(feature = "gvariant")]
            Format::GVariant => 1,
        }
    }
}

impl<'a> Type for Signature<'a> {
    fn signature() -> Signature<'static> {
        Signature::from_static_str_unchecked(Self::SIGNATURE_STR)
    }
}

impl<'a> From<&Signature<'a>> for Signature<'a> {
    fn from(signature: &Signature<'a>) -> Signature<'a> {
        signature.clone()
    }
}

impl<'a> TryFrom<&'a [u8]> for Signature<'a> {
    type Error = Error;

    fn try_from(value: &'a [u8]) -> Result<Self> {
        SignatureParser::validate(value)?;

        // SAFETY: validate checks UTF8
        unsafe { Ok(Self::from_bytes_unchecked(value)) }
    }
}

/// Try to create a Signature from a string.
impl<'a> TryFrom<&'a str> for Signature<'a> {
    type Error = Error;

    fn try_from(value: &'a str) -> Result<Self> {
        Self::try_from(value.as_bytes())
    }
}

/// Try to create a Signature from a `Cow<str>.`
impl<'a> TryFrom<Cow<'a, str>> for Signature<'a> {
    type Error = Error;

    fn try_from(value: Cow<'a, str>) -> Result<Self> {
        match value {
            Cow::Borrowed(v) => Self::try_from(v),
            Cow::Owned(v) => Self::try_from(v),
        }
    }
}

impl<'a> TryFrom<String> for Signature<'a> {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        SignatureParser::validate(value.as_bytes())?;

        Ok(Self::from_string_unchecked(value))
    }
}

impl<'a> From<Signature<'a>> for String {
    fn from(value: Signature<'a>) -> String {
        String::from(value.as_str())
    }
}

impl<'a> From<&Signature<'a>> for String {
    fn from(value: &Signature<'a>) -> String {
        String::from(value.as_str())
    }
}

impl<'a> std::ops::Deref for Signature<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

/// Checks whether the string slice has balanced parentheses.
fn has_balanced_parentheses(signature_str: &str) -> bool {
    signature_str.chars().fold(0, |count, ch| match ch {
        '(' => count + 1,
        ')' if count != 0 => count - 1,
        _ => count,
    }) == 0
}

/// Determines whether the signature has outer parentheses and if so, return the
/// string slice without those parentheses.
fn without_outer_parentheses<'a, 'b>(sig: &'a Signature<'b>) -> &'a str
where
    'b: 'a,
{
    let sig_str = sig.as_str();

    if let Some(subslice) = sig_str.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        if has_balanced_parentheses(subslice) {
            return subslice;
        }
    }
    sig_str
}

/// Evaluate equality of two signatures, ignoring outer parentheses if needed.
impl<'a, 'b> PartialEq<Signature<'a>> for Signature<'b> {
    fn eq(&self, other: &Signature<'_>) -> bool {
        without_outer_parentheses(self) == without_outer_parentheses(other)
    }
}

impl<'a> PartialEq<str> for Signature<'a> {
    fn eq(&self, other: &str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<'a> PartialEq<&str> for Signature<'a> {
    fn eq(&self, other: &&str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

// According to the docs, `Eq` derive should only be used on structs if all its fields are
// are `Eq`. Hence the manual implementation.
impl Eq for Signature<'_> {}

impl<'a> Display for Signature<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        std::fmt::Display::fmt(&self.as_str(), f)
    }
}

impl<'a> Serialize for Signature<'a> {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for Signature<'a> {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = <std::borrow::Cow<'a, str>>::deserialize(deserializer)?;

        Self::try_from(val).map_err(serde::de::Error::custom)
    }
}

/// Owned [`Signature`](struct.Signature.html)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, Type)]
pub struct OwnedSignature(Signature<'static>);

assert_impl_all!(OwnedSignature: Send, Sync, Unpin);

impl OwnedSignature {
    pub fn into_inner(self) -> Signature<'static> {
        self.0
    }
}

impl Basic for OwnedSignature {
    const SIGNATURE_CHAR: char = Signature::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = Signature::SIGNATURE_STR;

    fn alignment(format: Format) -> usize {
        Signature::alignment(format)
    }
}

impl std::ops::Deref for OwnedSignature {
    type Target = Signature<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::convert::From<OwnedSignature> for Signature<'static> {
    fn from(o: OwnedSignature) -> Self {
        o.into_inner()
    }
}

impl<'a> std::convert::From<Signature<'a>> for OwnedSignature {
    fn from(o: Signature<'a>) -> Self {
        OwnedSignature(o.into_owned())
    }
}

impl std::convert::From<OwnedSignature> for crate::Value<'static> {
    fn from(o: OwnedSignature) -> Self {
        o.into_inner().into()
    }
}

impl TryFrom<String> for OwnedSignature {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Ok(Self(Signature::try_from(value)?))
    }
}

impl<'de> Deserialize<'de> for OwnedSignature {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = String::deserialize(deserializer)?;

        OwnedSignature::try_from(val).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for OwnedSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.as_str(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::{Bytes, Signature};
    use std::sync::Arc;

    #[test]
    fn bytes_equality() {
        let borrowed1 = Bytes::Borrowed(b"foo");
        let borrowed2 = Bytes::Borrowed(b"foo");
        let static1 = Bytes::Static(b"foo");
        let static2 = Bytes::Static(b"foo");
        let owned1 = Bytes::Owned(Arc::new(*b"foo"));
        let owned2 = Bytes::Owned(Arc::new(*b"foo"));

        assert_eq!(borrowed1, borrowed2);
        assert_eq!(static1, static2);
        assert_eq!(owned1, owned2);

        assert_eq!(borrowed1, static1);
        assert_eq!(static1, borrowed1);

        assert_eq!(static1, owned1);
        assert_eq!(owned1, static1);

        assert_eq!(borrowed1, owned1);
        assert_eq!(owned1, borrowed1);
    }

    #[test]
    fn signature_slicing() {
        let sig = Signature::from_str_unchecked("(asta{sv})");
        assert_eq!(sig, "(asta{sv})");

        let slice = sig.slice(1..);
        assert_eq!(slice.len(), sig.len() - 1);
        assert_eq!(slice, &sig[1..]);
        assert_eq!(slice.as_bytes()[1], b's');
        assert_eq!(slice.as_bytes()[2], b't');

        let slice = slice.slice(2..3);
        assert_eq!(slice.len(), 1);
        assert_eq!(slice, "t");
        assert_eq!(slice.slice(1..), "");
    }

    #[test]
    fn signature_equality() {
        let sig_a = Signature::from_str_unchecked("(asta{sv})");
        let sig_b = Signature::from_str_unchecked("asta{sv}");
        assert_eq!(sig_a, sig_b);

        let sig_a = Signature::from_str_unchecked("((so)ii(uu))");
        let sig_b = Signature::from_str_unchecked("(so)ii(uu)");
        assert_eq!(sig_a, sig_b);

        let sig_a = Signature::from_str_unchecked("(so)i");
        let sig_b = Signature::from_str_unchecked("(so)u");
        assert_ne!(sig_a, sig_b);
    }
}
