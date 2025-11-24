use core::{fmt::Debug, str};
use serde::{
    de::{self, Deserialize, Deserializer, Visitor},
    ser::{Serialize, Serializer},
};
use static_assertions::assert_impl_all;
use std::borrow::Cow;

use crate::{serialized::Format, Basic, Error, Result, Signature, Str, Type};

/// String that identifies objects at a given destination on the D-Bus bus.
///
/// Mostly likely this is only useful in the D-Bus context.
///
/// # Examples
///
/// ```
/// use zvariant::ObjectPath;
///
/// // Valid object paths
/// let o = ObjectPath::try_from("/").unwrap();
/// assert_eq!(o, "/");
/// let o = ObjectPath::try_from("/Path/t0/0bject").unwrap();
/// assert_eq!(o, "/Path/t0/0bject");
/// let o = ObjectPath::try_from("/a/very/looooooooooooooooooooooooo0000o0ng/path").unwrap();
/// assert_eq!(o, "/a/very/looooooooooooooooooooooooo0000o0ng/path");
///
/// // Invalid object paths
/// ObjectPath::try_from("").unwrap_err();
/// ObjectPath::try_from("/double//slashes/").unwrap_err();
/// ObjectPath::try_from(".").unwrap_err();
/// ObjectPath::try_from("/end/with/slash/").unwrap_err();
/// ObjectPath::try_from("/ha.d").unwrap_err();
/// ```
#[derive(PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct ObjectPath<'a>(Str<'a>);

assert_impl_all!(ObjectPath<'_>: Send, Sync, Unpin);

impl<'a> ObjectPath<'a> {
    /// This is faster than `Clone::clone` when `self` contains owned data.
    pub fn as_ref(&self) -> ObjectPath<'_> {
        ObjectPath(self.0.as_ref())
    }

    /// The object path as a string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// The object path as bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Create a new `ObjectPath` from given bytes.
    ///
    /// Since the passed bytes are not checked for correctness, prefer using the
    /// `TryFrom<&[u8]>` implementation.
    ///
    /// # Safety
    ///
    /// See [`std::str::from_utf8_unchecked`].
    pub unsafe fn from_bytes_unchecked<'s: 'a>(bytes: &'s [u8]) -> Self {
        Self(std::str::from_utf8_unchecked(bytes).into())
    }

    /// Create a new `ObjectPath` from the given string.
    ///
    /// Since the passed string is not checked for correctness, prefer using the
    /// `TryFrom<&str>` implementation.
    pub fn from_str_unchecked<'s: 'a>(path: &'s str) -> Self {
        Self(path.into())
    }

    /// Same as `try_from`, except it takes a `&'static str`.
    pub fn from_static_str(name: &'static str) -> Result<Self> {
        ensure_correct_object_path_str(name.as_bytes())?;

        Ok(Self::from_static_str_unchecked(name))
    }

    /// Same as `from_str_unchecked`, except it takes a `&'static str`.
    pub const fn from_static_str_unchecked(name: &'static str) -> Self {
        Self(Str::from_static(name))
    }

    /// Same as `from_str_unchecked`, except it takes an owned `String`.
    ///
    /// Since the passed string is not checked for correctness, prefer using the
    /// `TryFrom<String>` implementation.
    pub fn from_string_unchecked(path: String) -> Self {
        Self(path.into())
    }

    /// the object path's length.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// if the object path is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Creates an owned clone of `self`.
    pub fn to_owned(&self) -> ObjectPath<'static> {
        ObjectPath(self.0.to_owned())
    }

    /// Creates an owned clone of `self`.
    pub fn into_owned(self) -> ObjectPath<'static> {
        ObjectPath(self.0.into_owned())
    }
}

impl std::default::Default for ObjectPath<'_> {
    fn default() -> Self {
        ObjectPath::from_static_str_unchecked("/")
    }
}

impl<'a> Basic for ObjectPath<'a> {
    const SIGNATURE_CHAR: char = 'o';
    const SIGNATURE_STR: &'static str = "o";

    fn alignment(format: Format) -> usize {
        match format {
            Format::DBus => <&str>::alignment(format),
            #[cfg(feature = "gvariant")]
            Format::GVariant => 1,
        }
    }
}

impl<'a> Type for ObjectPath<'a> {
    fn signature() -> Signature<'static> {
        Signature::from_static_str_unchecked(Self::SIGNATURE_STR)
    }
}

impl<'a> TryFrom<&'a [u8]> for ObjectPath<'a> {
    type Error = Error;

    fn try_from(value: &'a [u8]) -> Result<Self> {
        ensure_correct_object_path_str(value)?;

        // SAFETY: ensure_correct_object_path_str checks UTF-8
        unsafe { Ok(Self::from_bytes_unchecked(value)) }
    }
}

/// Try to create an ObjectPath from a string.
impl<'a> TryFrom<&'a str> for ObjectPath<'a> {
    type Error = Error;

    fn try_from(value: &'a str) -> Result<Self> {
        Self::try_from(value.as_bytes())
    }
}

impl<'a> TryFrom<String> for ObjectPath<'a> {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        ensure_correct_object_path_str(value.as_bytes())?;

        Ok(Self::from_string_unchecked(value))
    }
}

impl<'a> TryFrom<Cow<'a, str>> for ObjectPath<'a> {
    type Error = Error;

    fn try_from(value: Cow<'a, str>) -> Result<Self> {
        match value {
            Cow::Borrowed(s) => Self::try_from(s),
            Cow::Owned(s) => Self::try_from(s),
        }
    }
}

impl<'o> From<&ObjectPath<'o>> for ObjectPath<'o> {
    fn from(o: &ObjectPath<'o>) -> Self {
        o.clone()
    }
}

impl<'a> std::ops::Deref for ObjectPath<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<'a> PartialEq<str> for ObjectPath<'a> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<'a> PartialEq<&str> for ObjectPath<'a> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl<'a> Debug for ObjectPath<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ObjectPath").field(&self.as_str()).finish()
    }
}

impl<'a> std::fmt::Display for ObjectPath<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.as_str(), f)
    }
}

impl<'a> Serialize for ObjectPath<'a> {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for ObjectPath<'a> {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let visitor = ObjectPathVisitor;

        deserializer.deserialize_str(visitor)
    }
}

struct ObjectPathVisitor;

impl<'de> Visitor<'de> for ObjectPathVisitor {
    type Value = ObjectPath<'de>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("an ObjectPath")
    }

    #[inline]
    fn visit_borrowed_str<E>(self, value: &'de str) -> core::result::Result<ObjectPath<'de>, E>
    where
        E: serde::de::Error,
    {
        ObjectPath::try_from(value).map_err(serde::de::Error::custom)
    }
}

fn ensure_correct_object_path_str(path: &[u8]) -> Result<()> {
    let mut prev = b'\0';

    // Rules
    //
    // * At least 1 character.
    // * First character must be `/`
    // * No trailing `/`
    // * No `//`
    // * Only ASCII alphanumeric, `_` or '/'
    if path.is_empty() {
        return Err(serde::de::Error::invalid_length(0, &"> 0 character"));
    }

    for i in 0..path.len() {
        let c = path[i];

        if i == 0 && c != b'/' {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char(c as char),
                &"/",
            ));
        } else if c == b'/' && prev == b'/' {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str("//"),
                &"/",
            ));
        } else if path.len() > 1 && i == (path.len() - 1) && c == b'/' {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char('/'),
                &"an alphanumeric character or `_`",
            ));
        } else if !c.is_ascii_alphanumeric() && c != b'/' && c != b'_' {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char(c as char),
                &"an alphanumeric character, `_` or `/`",
            ));
        }
        prev = c;
    }

    Ok(())
}

/// Owned [`ObjectPath`](struct.ObjectPath.html)
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, serde::Serialize, Type)]
pub struct OwnedObjectPath(ObjectPath<'static>);

assert_impl_all!(OwnedObjectPath: Send, Sync, Unpin);

impl OwnedObjectPath {
    pub fn into_inner(self) -> ObjectPath<'static> {
        self.0
    }
}

impl Basic for OwnedObjectPath {
    const SIGNATURE_CHAR: char = ObjectPath::SIGNATURE_CHAR;
    const SIGNATURE_STR: &'static str = ObjectPath::SIGNATURE_STR;

    fn alignment(format: Format) -> usize {
        ObjectPath::alignment(format)
    }
}

impl std::ops::Deref for OwnedObjectPath {
    type Target = ObjectPath<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::convert::From<OwnedObjectPath> for ObjectPath<'static> {
    fn from(o: OwnedObjectPath) -> Self {
        o.into_inner()
    }
}

impl std::convert::From<OwnedObjectPath> for crate::Value<'_> {
    fn from(o: OwnedObjectPath) -> Self {
        o.into_inner().into()
    }
}

impl<'unowned, 'owned: 'unowned> From<&'owned OwnedObjectPath> for ObjectPath<'unowned> {
    fn from(o: &'owned OwnedObjectPath) -> Self {
        ObjectPath::from_str_unchecked(o.as_str())
    }
}

impl<'a> std::convert::From<ObjectPath<'a>> for OwnedObjectPath {
    fn from(o: ObjectPath<'a>) -> Self {
        OwnedObjectPath(o.into_owned())
    }
}

impl TryFrom<&'_ str> for OwnedObjectPath {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Ok(Self::from(ObjectPath::try_from(value)?))
    }
}

impl TryFrom<String> for OwnedObjectPath {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Ok(Self::from(ObjectPath::try_from(value)?))
    }
}

impl<'de> Deserialize<'de> for OwnedObjectPath {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)
            .and_then(|s| ObjectPath::try_from(s).map_err(|e| de::Error::custom(e.to_string())))
            .map(Self)
    }
}

impl std::fmt::Display for OwnedObjectPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.as_str(), f)
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn owned_from_reader() {
        // See https://github.com/dbus2/zbus/issues/287
        let json_str = "\"/some/path\"";
        serde_json::de::from_reader::<_, OwnedObjectPath>(json_str.as_bytes()).unwrap();
    }
}
