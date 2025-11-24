// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#[cfg(feature = "alloc")]
use alloc::borrow::Cow;
#[cfg(feature = "alloc")]
use alloc::borrow::ToOwned;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use core::cmp::Ordering;
use core::default::Default;
use core::fmt;
use core::fmt::Debug;
use core::hash::Hash;
use core::ops::Deref;
#[cfg(feature = "alloc")]
use zerovec::ule::VarULE;

pub use icu_locale_core::DataLocale;

/// The request type passed into all data provider implementations.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::exhaustive_structs)] // this type is stable
pub struct DataRequest<'a> {
    /// The data identifier for which to load data.
    ///
    /// If locale fallback is enabled, the resulting data may be from a different identifier
    /// than the one requested here.
    pub id: DataIdentifierBorrowed<'a>,
    /// Metadata that may affect the behavior of the data provider.
    pub metadata: DataRequestMetadata,
}

/// Metadata for data requests. This is currently empty, but it may be extended with options
/// for tuning locale fallback, buffer layout, and so forth.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub struct DataRequestMetadata {
    /// Silent requests do not log errors. This can be used for exploratory querying, such as fallbacks.
    pub silent: bool,
    /// Whether to allow prefix matches for the data marker attributes.
    pub attributes_prefix_match: bool,
}

/// The borrowed version of a [`DataIdentifierCow`].
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DataIdentifierBorrowed<'a> {
    /// Marker-specific request attributes
    pub marker_attributes: &'a DataMarkerAttributes,
    /// The CLDR locale
    pub locale: &'a DataLocale,
}

impl fmt::Display for DataIdentifierBorrowed<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self.locale, f)?;
        if !self.marker_attributes.is_empty() {
            write!(f, "/{}", self.marker_attributes.as_str())?;
        }
        Ok(())
    }
}

impl<'a> DataIdentifierBorrowed<'a> {
    /// Creates a [`DataIdentifierBorrowed`] for a borrowed [`DataLocale`].
    pub fn for_locale(locale: &'a DataLocale) -> Self {
        Self {
            locale,
            ..Default::default()
        }
    }

    /// Creates a [`DataIdentifierBorrowed`] for a borrowed [`DataMarkerAttributes`].
    pub fn for_marker_attributes(marker_attributes: &'a DataMarkerAttributes) -> Self {
        Self {
            marker_attributes,
            ..Default::default()
        }
    }

    /// Creates a [`DataIdentifierBorrowed`] for a borrowed [`DataMarkerAttributes`] and [`DataLocale`].
    pub fn for_marker_attributes_and_locale(
        marker_attributes: &'a DataMarkerAttributes,
        locale: &'a DataLocale,
    ) -> Self {
        Self {
            marker_attributes,
            locale,
        }
    }

    /// Converts this [`DataIdentifierBorrowed`] into a [`DataIdentifierCow<'static>`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn into_owned(self) -> DataIdentifierCow<'static> {
        DataIdentifierCow {
            marker_attributes: Cow::Owned(self.marker_attributes.to_owned()),
            locale: *self.locale,
        }
    }

    /// Borrows this [`DataIdentifierBorrowed`] as a [`DataIdentifierCow<'a>`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn as_cow(self) -> DataIdentifierCow<'a> {
        DataIdentifierCow {
            marker_attributes: Cow::Borrowed(self.marker_attributes),
            locale: *self.locale,
        }
    }
}

/// A data identifier identifies a particular version of data, such as "English".
///
/// It is a wrapper around a [`DataLocale`] and a [`DataMarkerAttributes`].
///
/// ✨ *Enabled with the `alloc` Cargo feature.*
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
#[non_exhaustive]
#[cfg(feature = "alloc")]
pub struct DataIdentifierCow<'a> {
    /// Marker-specific request attributes
    pub marker_attributes: Cow<'a, DataMarkerAttributes>,
    /// The CLDR locale
    pub locale: DataLocale,
}

#[cfg(feature = "alloc")]
impl PartialOrd for DataIdentifierCow<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(feature = "alloc")]
impl Ord for DataIdentifierCow<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.marker_attributes
            .cmp(&other.marker_attributes)
            .then_with(|| self.locale.total_cmp(&other.locale))
    }
}

#[cfg(feature = "alloc")]
impl fmt::Display for DataIdentifierCow<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.locale, f)?;
        if !self.marker_attributes.is_empty() {
            write!(f, "/{}", self.marker_attributes.as_str())?;
        }
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl<'a> DataIdentifierCow<'a> {
    /// Borrows this [`DataIdentifierCow`] as a [`DataIdentifierBorrowed<'a>`].
    pub fn as_borrowed(&'a self) -> DataIdentifierBorrowed<'a> {
        DataIdentifierBorrowed {
            marker_attributes: &self.marker_attributes,
            locale: &self.locale,
        }
    }

    /// Creates a [`DataIdentifierCow`] from an owned [`DataLocale`].
    pub fn from_locale(locale: DataLocale) -> Self {
        Self {
            marker_attributes: Cow::Borrowed(DataMarkerAttributes::empty()),
            locale,
        }
    }

    /// Creates a [`DataIdentifierCow`] from a borrowed [`DataMarkerAttributes`].
    pub fn from_marker_attributes(marker_attributes: &'a DataMarkerAttributes) -> Self {
        Self {
            marker_attributes: Cow::Borrowed(marker_attributes),
            locale: Default::default(),
        }
    }

    /// Creates a [`DataIdentifierCow`] from an owned [`DataMarkerAttributes`].
    pub fn from_marker_attributes_owned(marker_attributes: Box<DataMarkerAttributes>) -> Self {
        Self {
            marker_attributes: Cow::Owned(marker_attributes),
            locale: Default::default(),
        }
    }

    /// Creates a [`DataIdentifierCow`] from an owned [`DataMarkerAttributes`] and an owned [`DataLocale`].
    pub fn from_owned(marker_attributes: Box<DataMarkerAttributes>, locale: DataLocale) -> Self {
        Self {
            marker_attributes: Cow::Owned(marker_attributes),
            locale,
        }
    }

    /// Creates a [`DataIdentifierCow`] from a borrowed [`DataMarkerAttributes`] and an owned [`DataLocale`].
    pub fn from_borrowed_and_owned(
        marker_attributes: &'a DataMarkerAttributes,
        locale: DataLocale,
    ) -> Self {
        Self {
            marker_attributes: Cow::Borrowed(marker_attributes),
            locale,
        }
    }

    /// Returns whether this id is equal to the default.
    pub fn is_unknown(&self) -> bool {
        self.marker_attributes.is_empty() && self.locale.is_unknown()
    }
}

#[cfg(feature = "alloc")]
impl Default for DataIdentifierCow<'_> {
    fn default() -> Self {
        Self {
            marker_attributes: Cow::Borrowed(Default::default()),
            locale: Default::default(),
        }
    }
}

/// An additional key to identify data beyond a [`DataLocale`].
///
/// The is a loose wrapper around a string, with semantics defined by each [`DataMarker`](crate::DataMarker).
#[derive(PartialEq, Eq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct DataMarkerAttributes {
    // Validated to be non-empty ASCII alphanumeric + hyphen + underscore
    value: str,
}

impl Default for &DataMarkerAttributes {
    fn default() -> Self {
        DataMarkerAttributes::empty()
    }
}

impl Deref for DataMarkerAttributes {
    type Target = str;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl Debug for DataMarkerAttributes {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

/// Invalid character
#[derive(Debug)]
#[non_exhaustive]
pub struct AttributeParseError;

impl DataMarkerAttributes {
    /// Safety-usable invariant: validated bytes are ASCII only
    const fn validate(s: &[u8]) -> Result<(), AttributeParseError> {
        let mut i = 0;
        while i < s.len() {
            #[expect(clippy::indexing_slicing)] // duh
            if !matches!(s[i], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_') {
                return Err(AttributeParseError);
            }
            i += 1;
        }
        Ok(())
    }

    /// Creates a borrowed [`DataMarkerAttributes`] from a borrowed string.
    ///
    /// Returns an error if the string contains characters other than `[a-zA-Z0-9_\-]`.
    pub const fn try_from_str(s: &str) -> Result<&Self, AttributeParseError> {
        Self::try_from_utf8(s.as_bytes())
    }

    /// Attempts to create a borrowed [`DataMarkerAttributes`] from a borrowed UTF-8 encoded byte slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu_provider::prelude::*;
    ///
    /// let bytes = b"long-meter";
    /// let marker = DataMarkerAttributes::try_from_utf8(bytes).unwrap();
    /// assert_eq!(marker.to_string(), "long-meter");
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the byte slice contains code units other than `[a-zA-Z0-9_\-]`.
    pub const fn try_from_utf8(code_units: &[u8]) -> Result<&Self, AttributeParseError> {
        let Ok(()) = Self::validate(code_units) else {
            return Err(AttributeParseError);
        };

        // SAFETY: `validate` requires a UTF-8 subset
        let s = unsafe { core::str::from_utf8_unchecked(code_units) };

        // SAFETY: `Self` has the same layout as `str`
        Ok(unsafe { &*(s as *const str as *const Self) })
    }

    /// Creates an owned [`DataMarkerAttributes`] from an owned string.
    ///
    /// Returns an error if the string contains characters other than `[a-zA-Z0-9_\-]`.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn try_from_string(s: String) -> Result<Box<Self>, AttributeParseError> {
        let Ok(()) = Self::validate(s.as_bytes()) else {
            return Err(AttributeParseError);
        };

        // SAFETY: `Self` has the same layout as `str`
        Ok(unsafe { core::mem::transmute::<Box<str>, Box<Self>>(s.into_boxed_str()) })
    }

    /// Creates a borrowed [`DataMarkerAttributes`] from a borrowed string.
    ///
    /// Panics if the string contains characters other than `[a-zA-Z0-9_\-]`.
    pub const fn from_str_or_panic(s: &str) -> &Self {
        let Ok(r) = Self::try_from_str(s) else {
            panic!("Invalid marker attribute syntax")
        };
        r
    }

    /// Creates an empty [`DataMarkerAttributes`].
    pub const fn empty() -> &'static Self {
        // SAFETY: `Self` has the same layout as `str`
        unsafe { &*("" as *const str as *const Self) }
    }

    /// Returns this [`DataMarkerAttributes`] as a `&str`.
    pub const fn as_str(&self) -> &str {
        &self.value
    }
}

/// ✨ *Enabled with the `alloc` Cargo feature.*
#[cfg(feature = "alloc")]
impl ToOwned for DataMarkerAttributes {
    type Owned = Box<Self>;
    fn to_owned(&self) -> Self::Owned {
        // SAFETY: `Self` has the same layout as `str`
        unsafe { core::mem::transmute::<Box<str>, Box<Self>>(self.as_str().to_boxed()) }
    }
}

#[test]
fn test_data_marker_attributes_from_utf8() {
    let bytes_vec: Vec<&[u8]> = vec![
        b"long-meter",
        b"long",
        b"meter",
        b"short-meter-second",
        b"usd",
    ];

    for bytes in bytes_vec {
        let marker = DataMarkerAttributes::try_from_utf8(bytes).unwrap();
        assert_eq!(marker.to_string().as_bytes(), bytes);
    }
}
