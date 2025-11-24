use serde::{Deserialize, Deserializer, Serialize, Serializer};
use static_assertions::assert_impl_all;
use std::{
    borrow::Cow,
    cmp::Ordering,
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::{serialized::Format, Basic, Signature, Type};

/// A string wrapper.
///
/// This is used for keeping strings in a [`Value`]. API is provided to convert from, and to a
/// [`&str`] and [`String`].
///
/// [`Value`]: enum.Value.html#variant.Str
/// [`&str`]: https://doc.rust-lang.org/std/str/index.html
/// [`String`]: https://doc.rust-lang.org/std/string/struct.String.html
#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Serialize, Deserialize)]
#[serde(rename(serialize = "zvariant::Str", deserialize = "zvariant::Str"))]
pub struct Str<'a>(#[serde(borrow)] Inner<'a>);

#[derive(Eq, Clone)]
enum Inner<'a> {
    Static(&'static str),
    Borrowed(&'a str),
    Owned(Arc<str>),
}

impl<'a> Default for Inner<'a> {
    fn default() -> Self {
        Self::Static("")
    }
}

impl<'a> PartialEq for Inner<'a> {
    fn eq(&self, other: &Inner<'a>) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<'a> Ord for Inner<'a> {
    fn cmp(&self, other: &Inner<'a>) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl<'a> PartialOrd for Inner<'a> {
    fn partial_cmp(&self, other: &Inner<'a>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Hash for Inner<'a> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.as_str().hash(h)
    }
}

impl<'a> Inner<'a> {
    /// The underlying string.
    pub fn as_str(&self) -> &str {
        match self {
            Inner::Static(s) => s,
            Inner::Borrowed(s) => s,
            Inner::Owned(s) => s,
        }
    }
}

impl<'a> Serialize for Inner<'a> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for Inner<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        <&'a str>::deserialize(deserializer).map(Inner::Borrowed)
    }
}

assert_impl_all!(Str<'_>: Send, Sync, Unpin);

impl<'a> Str<'a> {
    /// An owned string without allocations
    pub const fn from_static(s: &'static str) -> Self {
        Str(Inner::Static(s))
    }

    /// This is faster than `Clone::clone` when `self` contains owned data.
    pub fn as_ref(&self) -> Str<'_> {
        match &self.0 {
            Inner::Static(s) => Str(Inner::Static(s)),
            Inner::Borrowed(s) => Str(Inner::Borrowed(s)),
            Inner::Owned(s) => Str(Inner::Borrowed(s)),
        }
    }

    /// The underlying string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Creates an owned clone of `self`.
    pub fn to_owned(&self) -> Str<'static> {
        self.clone().into_owned()
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> Str<'static> {
        match self.0 {
            Inner::Static(s) => Str(Inner::Static(s)),
            Inner::Borrowed(s) => Str(Inner::Owned(s.to_owned().into())),
            Inner::Owned(s) => Str(Inner::Owned(s)),
        }
    }
}

impl<'a> Basic for Str<'a> {
    const SIGNATURE_CHAR: char = <&str>::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = <&str>::SIGNATURE_STR;

    fn alignment(format: Format) -> usize {
        <&str>::alignment(format)
    }
}

impl<'a> Type for Str<'a> {
    fn signature() -> Signature<'static> {
        Signature::from_static_str_unchecked(Self::SIGNATURE_STR)
    }
}

impl<'a> From<&'a str> for Str<'a> {
    fn from(value: &'a str) -> Self {
        Self(Inner::Borrowed(value))
    }
}

impl<'a> From<&'a String> for Str<'a> {
    fn from(value: &'a String) -> Self {
        Self(Inner::Borrowed(value))
    }
}

impl<'a> From<String> for Str<'a> {
    fn from(value: String) -> Self {
        Self(Inner::Owned(value.into()))
    }
}

impl<'a> From<Arc<str>> for Str<'a> {
    fn from(value: Arc<str>) -> Self {
        Self(Inner::Owned(value))
    }
}

impl<'a> From<Cow<'a, str>> for Str<'a> {
    fn from(value: Cow<'a, str>) -> Self {
        match value {
            Cow::Owned(value) => value.into(),
            Cow::Borrowed(value) => value.into(),
        }
    }
}

impl<'a> From<Str<'a>> for String {
    fn from(value: Str<'a>) -> String {
        match value.0 {
            Inner::Static(s) => s.into(),
            Inner::Borrowed(s) => s.into(),
            Inner::Owned(s) => s.to_string(),
        }
    }
}

impl<'a> From<&'a Str<'_>> for &'a str {
    fn from(value: &'a Str<'_>) -> &'a str {
        value.as_str()
    }
}

impl<'a> std::ops::Deref for Str<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<'a> PartialEq<str> for Str<'a> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<'a> PartialEq<&str> for Str<'a> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl<'a> std::fmt::Debug for Str<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl<'a> std::fmt::Display for Str<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.as_str(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::Str;

    #[test]
    fn from_string() {
        let string = String::from("value");
        let v = Str::from(&string);
        assert_eq!(v.as_str(), "value");
    }

    #[test]
    fn test_ordering() {
        let first = Str::from("a".to_string());
        let second = Str::from_static("b");
        assert!(first < second);
    }
}
