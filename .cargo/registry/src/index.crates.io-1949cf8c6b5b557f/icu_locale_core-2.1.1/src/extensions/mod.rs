// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Unicode Extensions provide a mechanism to extend the [`LanguageIdentifier`] with
//! additional bits of information - a combination of a [`LanguageIdentifier`] and [`Extensions`]
//! is called [`Locale`].
//!
//! There are four types of extensions:
//!
//!  * [`Unicode Extensions`] - marked as `u`.
//!  * [`Transform Extensions`] - marked as `t`.
//!  * [`Private Use Extensions`] - marked as `x`.
//!  * [`Other Extensions`] - marked as any `a-z` except of `u`, `t` and `x`.
//!
//! One can think of extensions as a bag of extra information on top of basic 4 [`subtags`].
//!
//! Notice: `Other` extension type is currently not supported.
//!
//! # Examples
//!
//! ```
//! use icu::locale::extensions::unicode::{Key, Value};
//! use icu::locale::Locale;
//!
//! let loc: Locale = "en-US-u-ca-buddhist-t-en-us-h0-hybrid-x-foo"
//!     .parse()
//!     .expect("Failed to parse.");
//!
//! assert_eq!(loc.id.language, "en".parse().unwrap());
//! assert_eq!(loc.id.script, None);
//! assert_eq!(loc.id.region, Some("US".parse().unwrap()));
//! assert_eq!(loc.id.variants.len(), 0);
//!
//! let key: Key = "ca".parse().expect("Parsing key failed.");
//! let value: Value = "buddhist".parse().expect("Parsing value failed.");
//! assert_eq!(loc.extensions.unicode.keywords.get(&key), Some(&value));
//! ```
//!
//! # Syntactic vs Semantic Extension Handling
//!
//! This module is useful when you need to work with Locale extensions at a syntactic level,
//! perhaps for parsing or generating locale identifiers that include any syntactically valid
//! extensions.
//! For handling and validating known CLDR values with semantic meaning, see the
//! [`crate::preferences::extensions`] module.
//!
//! [`LanguageIdentifier`]: super::LanguageIdentifier
//! [`Locale`]: super::Locale
//! [`subtags`]: super::subtags
//! [`Other Extensions`]: other
//! [`Private Use Extensions`]: private
//! [`Transform Extensions`]: transform
//! [`Unicode Extensions`]: unicode
pub mod other;
pub mod private;
pub mod transform;
pub mod unicode;

use core::cmp::Ordering;

use other::Other;
use private::{Private, PRIVATE_EXT_CHAR};
use transform::{Transform, TRANSFORM_EXT_CHAR};
use unicode::{Unicode, UNICODE_EXT_CHAR};

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::parser::ParseError;
#[cfg(feature = "alloc")]
use crate::parser::SubtagIterator;
use crate::subtags;

/// Defines the type of extension.
#[derive(Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord, Copy)]
#[non_exhaustive]
pub enum ExtensionType {
    /// Transform Extension Type marked as `t`.
    Transform,
    /// Unicode Extension Type marked as `u`.
    Unicode,
    /// Private Extension Type marked as `x`.
    Private,
    /// All other extension types.
    Other(u8),
}

impl ExtensionType {
    #[allow(dead_code)]
    pub(crate) const fn try_from_byte_slice(key: &[u8]) -> Result<Self, ParseError> {
        if let [b] = key {
            Self::try_from_byte(*b)
        } else {
            Err(ParseError::InvalidExtension)
        }
    }

    pub(crate) const fn try_from_byte(key: u8) -> Result<Self, ParseError> {
        let key = key.to_ascii_lowercase();
        match key as char {
            UNICODE_EXT_CHAR => Ok(Self::Unicode),
            TRANSFORM_EXT_CHAR => Ok(Self::Transform),
            PRIVATE_EXT_CHAR => Ok(Self::Private),
            'a'..='z' => Ok(Self::Other(key)),
            _ => Err(ParseError::InvalidExtension),
        }
    }

    pub(crate) const fn try_from_utf8(code_units: &[u8]) -> Result<Self, ParseError> {
        let &[first] = code_units else {
            return Err(ParseError::InvalidExtension);
        };

        Self::try_from_byte(first)
    }
}

/// A map of extensions associated with a given [`Locale`](crate::Locale).
#[derive(Debug, Default, PartialEq, Eq, Clone, Hash)]
#[non_exhaustive]
pub struct Extensions {
    /// A representation of the data for a Unicode extension, when present in the locale identifier.
    pub unicode: Unicode,
    /// A representation of the data for a transform extension, when present in the locale identifier.
    pub transform: Transform,
    /// A representation of the data for a private-use extension, when present in the locale identifier.
    pub private: Private,
    /// A sequence of any other extensions that are present in the locale identifier but are not formally
    /// [defined](https://unicode.org/reports/tr35/) and represented explicitly as [`Unicode`], [`Transform`],
    /// and [`Private`] are.
    #[cfg(feature = "alloc")]
    pub other: Vec<Other>,
    /// A sequence of any other extensions that are present in the locale identifier but are not formally
    /// [defined](https://unicode.org/reports/tr35/) and represented explicitly as [`Unicode`], [`Transform`],
    /// and [`Private`] are.
    #[cfg(not(feature = "alloc"))]
    pub other: &'static [Other],
}

impl Extensions {
    /// Returns a new empty map of extensions. Same as [`default()`](Default::default()), but is `const`.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::Extensions;
    ///
    /// assert_eq!(Extensions::new(), Extensions::default());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            unicode: Unicode::new(),
            transform: Transform::new(),
            private: Private::new(),
            #[cfg(feature = "alloc")]
            other: Vec::new(),
            #[cfg(not(feature = "alloc"))]
            other: &[],
        }
    }

    /// Function to create a new map of extensions containing exactly one unicode extension, callable in `const`
    /// context.
    #[inline]
    pub const fn from_unicode(unicode: Unicode) -> Self {
        Self {
            unicode,
            transform: Transform::new(),
            private: Private::new(),
            #[cfg(feature = "alloc")]
            other: Vec::new(),
            #[cfg(not(feature = "alloc"))]
            other: &[],
        }
    }

    /// Returns whether there are no extensions present.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    ///
    /// let loc: Locale = "en-US-u-foo".parse().expect("Parsing failed.");
    ///
    /// assert!(!loc.extensions.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.unicode.is_empty()
            && self.transform.is_empty()
            && self.private.is_empty()
            && self.other.is_empty()
    }

    #[expect(clippy::type_complexity)]
    pub(crate) fn as_tuple(
        &self,
    ) -> (
        (&unicode::Attributes, &unicode::Keywords),
        (
            Option<(
                subtags::Language,
                Option<subtags::Script>,
                Option<subtags::Region>,
                &subtags::Variants,
            )>,
            &transform::Fields,
        ),
        &private::Private,
        &[other::Other],
    ) {
        (
            self.unicode.as_tuple(),
            self.transform.as_tuple(),
            &self.private,
            &self.other,
        )
    }

    /// Returns an ordering suitable for use in [`BTreeSet`].
    ///
    /// The ordering may or may not be equivalent to string ordering, and it
    /// may or may not be stable across ICU4X releases.
    ///
    /// [`BTreeSet`]: alloc::collections::BTreeSet
    pub fn total_cmp(&self, other: &Self) -> Ordering {
        self.as_tuple().cmp(&other.as_tuple())
    }

    /// Retains the specified extension types, clearing all others.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::ExtensionType;
    /// use icu::locale::Locale;
    ///
    /// let loc: Locale =
    ///     "und-a-hello-t-mul-u-world-z-zzz-x-extra".parse().unwrap();
    ///
    /// let mut only_unicode = loc.clone();
    /// only_unicode
    ///     .extensions
    ///     .retain_by_type(|t| t == ExtensionType::Unicode);
    /// assert_eq!(only_unicode, "und-u-world".parse().unwrap());
    ///
    /// let mut only_t_z = loc.clone();
    /// only_t_z.extensions.retain_by_type(|t| {
    ///     t == ExtensionType::Transform || t == ExtensionType::Other(b'z')
    /// });
    /// assert_eq!(only_t_z, "und-t-mul-z-zzz".parse().unwrap());
    /// ```
    #[cfg(feature = "alloc")]
    pub fn retain_by_type<F>(&mut self, mut predicate: F)
    where
        F: FnMut(ExtensionType) -> bool,
    {
        if !predicate(ExtensionType::Unicode) {
            self.unicode.clear();
        }
        if !predicate(ExtensionType::Transform) {
            self.transform.clear();
        }
        if !predicate(ExtensionType::Private) {
            self.private.clear();
        }
        #[cfg(feature = "alloc")]
        self.other
            .retain(|o| predicate(ExtensionType::Other(o.get_ext_byte())));
    }

    #[cfg(feature = "alloc")]
    pub(crate) fn try_from_iter(iter: &mut SubtagIterator) -> Result<Self, ParseError> {
        let mut unicode = None;
        let mut transform = None;
        let mut private = None;
        let mut other = Vec::new();

        while let Some(subtag) = iter.next() {
            if subtag.is_empty() {
                return Err(ParseError::InvalidExtension);
            }

            let &[subtag] = subtag else {
                return Err(ParseError::InvalidExtension);
            };

            match ExtensionType::try_from_byte(subtag) {
                Ok(ExtensionType::Unicode) => {
                    if unicode.is_some() {
                        return Err(ParseError::DuplicatedExtension);
                    }
                    unicode = Some(Unicode::try_from_iter(iter)?);
                }
                Ok(ExtensionType::Transform) => {
                    if transform.is_some() {
                        return Err(ParseError::DuplicatedExtension);
                    }
                    transform = Some(Transform::try_from_iter(iter)?);
                }
                Ok(ExtensionType::Private) => {
                    if private.is_some() {
                        return Err(ParseError::DuplicatedExtension);
                    }
                    private = Some(Private::try_from_iter(iter)?);
                }
                Ok(ExtensionType::Other(ext)) => {
                    if other.iter().any(|o: &Other| o.get_ext_byte() == ext) {
                        return Err(ParseError::DuplicatedExtension);
                    }
                    let parsed = Other::try_from_iter(ext, iter)?;
                    if let Err(idx) = other.binary_search(&parsed) {
                        other.insert(idx, parsed);
                    } else {
                        return Err(ParseError::InvalidExtension);
                    }
                }
                _ => return Err(ParseError::InvalidExtension),
            }
        }

        Ok(Self {
            unicode: unicode.unwrap_or_default(),
            transform: transform.unwrap_or_default(),
            private: private.unwrap_or_default(),
            other,
        })
    }

    pub(crate) fn for_each_subtag_str<E, F>(&self, f: &mut F) -> Result<(), E>
    where
        F: FnMut(&str) -> Result<(), E>,
    {
        let mut wrote_tu = false;
        // Alphabetic by singleton
        self.other.iter().try_for_each(|other| {
            if other.get_ext() > TRANSFORM_EXT_CHAR && !wrote_tu {
                // Since 't' and 'u' are next to each other in alphabetical
                // order, write both now.
                self.transform.for_each_subtag_str(f, true)?;
                self.unicode.for_each_subtag_str(f, true)?;
                wrote_tu = true;
            }
            other.for_each_subtag_str(f, true)?;
            Ok(())
        })?;

        if !wrote_tu {
            self.transform.for_each_subtag_str(f, true)?;
            self.unicode.for_each_subtag_str(f, true)?;
        }

        // Private must be written last, since it allows single character
        // keys. Extensions must also be written in alphabetical order,
        // which would seem to imply that other extensions `y` and `z` are
        // invalid, but this is not specified.
        self.private.for_each_subtag_str(f, true)?;
        Ok(())
    }
}

impl_writeable_for_each_subtag_str_no_test!(Extensions);

#[test]
fn test_writeable() {
    use crate::Locale;
    use writeable::assert_writeable_eq;
    assert_writeable_eq!(Extensions::new(), "");
    assert_writeable_eq!(
        "my-t-my-d0-zawgyi".parse::<Locale>().unwrap().extensions,
        "t-my-d0-zawgyi",
    );
    assert_writeable_eq!(
        "ar-SA-u-ca-islamic-civil"
            .parse::<Locale>()
            .unwrap()
            .extensions,
        "u-ca-islamic-civil",
    );
    assert_writeable_eq!(
        "en-001-x-foo-bar".parse::<Locale>().unwrap().extensions,
        "x-foo-bar",
    );
    assert_writeable_eq!(
        "und-t-m0-true".parse::<Locale>().unwrap().extensions,
        "t-m0-true",
    );
    assert_writeable_eq!(
        "und-a-foo-t-foo-u-foo-w-foo-z-foo-x-foo"
            .parse::<Locale>()
            .unwrap()
            .extensions,
        "a-foo-t-foo-u-foo-w-foo-z-foo-x-foo",
    );
}
