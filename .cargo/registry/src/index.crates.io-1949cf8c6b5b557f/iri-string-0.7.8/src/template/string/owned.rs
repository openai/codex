//! Owned `UriTemplateString`.

use core::fmt;

use alloc::borrow::Cow;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::borrow::ToOwned;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::boxed::Box;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::String;

use crate::template::error::{CreationError, Error, ErrorKind};
use crate::template::parser::validate_template_str;
use crate::template::string::UriTemplateStr;

/// An owned slice of a URI template.
///
/// URI Template is defined by [RFC 6570].
///
/// Note that "URI Template" can also be used for IRI.
///
/// [RFC 6570]: https://www.rfc-editor.org/rfc/rfc6570.html
///
/// # Valid values
///
/// This type can have a URI template string.
// Note that `From<$ty> for {Arc,Rc}<$slice>` is currently not implemented since
// this won't reuse allocated memory and hides internal memory reallocation. See
// <https://github.com/lo48576/iri-string/issues/20#issuecomment-1105207849>.
// However, this is not decided with firm belief or opinion, so there would be
// a chance that they are implemented in future.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UriTemplateString {
    /// Inner data.
    inner: String,
}

impl UriTemplateString {
    /// Creates a new string without validation.
    ///
    /// This does not validate the given string, so it is caller's
    /// responsibility to ensure the given string is valid.
    ///
    /// # Safety
    ///
    /// The given string must be syntactically valid as `Self` type.
    /// If not, any use of the returned value or the call of this
    /// function itself may result in undefined behavior.
    #[inline]
    #[must_use]
    pub unsafe fn new_unchecked(s: alloc::string::String) -> Self {
        // The construction itself can be written in safe Rust, but
        // every other place including unsafe functions expects
        // `self.inner` to be syntactically valid as `Self`. In order to
        // make them safe, the construction should validate the value
        // or at least should require users to validate the value by
        // making the function `unsafe`.
        Self { inner: s }
    }

    /// Shrinks the capacity of the inner buffer to match its length.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit()
    }

    /// Returns the internal buffer capacity in bytes.
    #[inline]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Returns the borrowed IRI string slice.
    ///
    /// This is equivalent to `&*self`.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &UriTemplateStr {
        self.as_ref()
    }

    /// Appends the template string.
    #[inline]
    pub fn append(&mut self, other: &UriTemplateStr) {
        self.inner.push_str(other.as_str());
        debug_assert!(validate_template_str(self.as_str()).is_ok());
    }
}

impl AsRef<str> for UriTemplateString {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.inner
    }
}

impl AsRef<UriTemplateStr> for UriTemplateString {
    #[inline]
    fn as_ref(&self) -> &UriTemplateStr {
        // SAFETY: `UriTemplateString and `UriTemplateStr` requires same validation,
        // so the content of `self: &UriTemplateString` must be valid as `UriTemplateStr`.
        unsafe { UriTemplateStr::new_always_unchecked(AsRef::<str>::as_ref(self)) }
    }
}

impl core::borrow::Borrow<str> for UriTemplateString {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_ref()
    }
}

impl core::borrow::Borrow<UriTemplateStr> for UriTemplateString {
    #[inline]
    fn borrow(&self) -> &UriTemplateStr {
        self.as_ref()
    }
}

impl ToOwned for UriTemplateStr {
    type Owned = UriTemplateString;

    #[inline]
    fn to_owned(&self) -> Self::Owned {
        self.into()
    }
}

impl From<&'_ UriTemplateStr> for UriTemplateString {
    #[inline]
    fn from(s: &UriTemplateStr) -> Self {
        // This is safe because `s` must be valid.
        Self {
            inner: alloc::string::String::from(s.as_str()),
        }
    }
}

impl From<UriTemplateString> for alloc::string::String {
    #[inline]
    fn from(s: UriTemplateString) -> Self {
        s.inner
    }
}

impl<'a> From<UriTemplateString> for Cow<'a, UriTemplateStr> {
    #[inline]
    fn from(s: UriTemplateString) -> Cow<'a, UriTemplateStr> {
        Cow::Owned(s)
    }
}

impl From<UriTemplateString> for Box<UriTemplateStr> {
    #[inline]
    fn from(s: UriTemplateString) -> Box<UriTemplateStr> {
        let inner: String = s.into();
        let buf = Box::<str>::from(inner);
        // SAFETY: `UriTemplateStr` has `repr(transparent)` attribute, so
        // the memory layouts of `Box<str>` and `Box<UriTemplateStr>` are
        // compatible. Additionally, `UriTemplateString` and `UriTemplateStr`
        // require the same syntax.
        unsafe {
            let raw: *mut str = Box::into_raw(buf);
            Box::<UriTemplateStr>::from_raw(raw as *mut UriTemplateStr)
        }
    }
}

impl TryFrom<&'_ str> for UriTemplateString {
    type Error = Error;

    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        <&UriTemplateStr>::try_from(s).map(Into::into)
    }
}

impl TryFrom<&'_ [u8]> for UriTemplateString {
    type Error = Error;

    #[inline]
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let s = core::str::from_utf8(bytes)
            .map_err(|e| Error::new(ErrorKind::InvalidUtf8, e.valid_up_to()))?;
        <&UriTemplateStr>::try_from(s).map(Into::into)
    }
}

impl core::convert::TryFrom<alloc::string::String> for UriTemplateString {
    type Error = CreationError<String>;

    #[inline]
    fn try_from(s: alloc::string::String) -> Result<Self, Self::Error> {
        match <&UriTemplateStr>::try_from(s.as_str()) {
            Ok(_) => {
                // This is safe because `<&UriTemplateStr>::try_from(s)?` ensures
                // that the string `s` is valid.
                Ok(Self { inner: s })
            }
            Err(e) => Err(CreationError::new(e, s)),
        }
    }
}

impl alloc::str::FromStr for UriTemplateString {
    type Err = Error;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TryFrom::try_from(s)
    }
}

impl core::ops::Deref for UriTemplateString {
    type Target = UriTemplateStr;

    #[inline]
    fn deref(&self) -> &UriTemplateStr {
        self.as_ref()
    }
}

impl_cmp!(str, UriTemplateStr, Cow<'_, str>);
impl_cmp!(str, &UriTemplateStr, Cow<'_, str>);

impl_cmp!(str, str, UriTemplateString);
impl_cmp!(str, &str, UriTemplateString);
impl_cmp!(str, Cow<'_, str>, UriTemplateString);
impl_cmp!(str, String, UriTemplateString);

impl fmt::Display for UriTemplateString {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Serde deserializer implementation.
#[cfg(feature = "serde")]
mod __serde_owned {
    use super::UriTemplateString;

    use core::fmt;

    #[cfg(all(feature = "alloc", feature = "serde", not(feature = "std")))]
    use alloc::string::String;

    use serde::{
        de::{self, Visitor},
        Deserialize, Deserializer,
    };

    /// Custom owned string visitor.
    #[derive(Debug, Clone, Copy)]
    struct CustomStringVisitor;

    impl Visitor<'_> for CustomStringVisitor {
        type Value = UriTemplateString;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("URI template string")
        }

        #[inline]
        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            <UriTemplateString as TryFrom<&str>>::try_from(v).map_err(E::custom)
        }

        #[cfg(feature = "serde")]
        #[inline]
        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            <UriTemplateString as TryFrom<String>>::try_from(v).map_err(E::custom)
        }
    }

    impl<'de> Deserialize<'de> for UriTemplateString {
        #[inline]
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_str(CustomStringVisitor)
        }
    }
}
