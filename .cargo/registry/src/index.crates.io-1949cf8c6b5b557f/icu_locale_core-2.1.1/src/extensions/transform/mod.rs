// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Transform Extensions provide information on content transformations in a given locale.
//!
//! The main struct for this extension is [`Transform`] which contains [`Fields`] and an
//! optional [`LanguageIdentifier`].
//!
//! [`LanguageIdentifier`]: super::super::LanguageIdentifier
//!
//! # Examples
//!
//! ```
//! use icu::locale::extensions::transform::{Fields, Key, Transform, Value};
//! use icu::locale::{LanguageIdentifier, Locale};
//!
//! let mut loc: Locale =
//!     "en-US-t-es-ar-h0-hybrid".parse().expect("Parsing failed.");
//!
//! let lang: LanguageIdentifier =
//!     "es-AR".parse().expect("Parsing LanguageIdentifier failed.");
//!
//! let key: Key = "h0".parse().expect("Parsing key failed.");
//! let value: Value = "hybrid".parse().expect("Parsing value failed.");
//!
//! assert_eq!(loc.extensions.transform.lang, Some(lang));
//! assert!(loc.extensions.transform.fields.contains_key(&key));
//! assert_eq!(loc.extensions.transform.fields.get(&key), Some(&value));
//!
//! assert_eq!(&loc.extensions.transform.to_string(), "t-es-ar-h0-hybrid");
//! ```
mod fields;
mod key;
mod value;

use core::cmp::Ordering;
#[cfg(feature = "alloc")]
use core::str::FromStr;

pub use fields::Fields;
#[doc(inline)]
pub use key::{key, Key};
pub use value::Value;

#[cfg(feature = "alloc")]
use super::ExtensionType;
#[cfg(feature = "alloc")]
use crate::parser::SubtagIterator;
#[cfg(feature = "alloc")]
use crate::parser::{parse_language_identifier_from_iter, ParseError, ParserMode};
#[cfg(feature = "alloc")]
use crate::shortvec::ShortBoxSlice;
use crate::subtags;
#[cfg(feature = "alloc")]
use crate::subtags::Language;
use crate::LanguageIdentifier;
#[cfg(feature = "alloc")]
use litemap::LiteMap;

pub(crate) const TRANSFORM_EXT_CHAR: char = 't';
pub(crate) const TRANSFORM_EXT_STR: &str = "t";

/// A list of [`Unicode BCP47 T Extensions`] as defined in [`Unicode Locale
/// Identifier`] specification.
///
/// Transform extension carries information about source language or script of
/// transformed content, including content that has been transliterated, transcribed,
/// or translated, or in some other way influenced by the source (See [`RFC 6497`] for details).
///
/// # Examples
///
/// ```
/// use icu::locale::extensions::transform::{Key, Value};
/// use icu::locale::{LanguageIdentifier, Locale};
///
/// let mut loc: Locale =
///     "de-t-en-us-h0-hybrid".parse().expect("Parsing failed.");
///
/// let en_us: LanguageIdentifier = "en-US".parse().expect("Parsing failed.");
///
/// assert_eq!(loc.extensions.transform.lang, Some(en_us));
/// let key: Key = "h0".parse().expect("Parsing key failed.");
/// let value: Value = "hybrid".parse().expect("Parsing value failed.");
/// assert_eq!(loc.extensions.transform.fields.get(&key), Some(&value));
/// ```
/// [`Unicode BCP47 T Extensions`]: https://unicode.org/reports/tr35/#t_Extension
/// [`RFC 6497`]: https://www.ietf.org/rfc/rfc6497.txt
/// [`Unicode Locale Identifier`]: https://unicode.org/reports/tr35/#Unicode_locale_identifier
#[derive(Clone, PartialEq, Eq, Debug, Default, Hash)]
#[allow(clippy::exhaustive_structs)] // spec-backed stable datastructure
pub struct Transform {
    /// The [`LanguageIdentifier`] specified with this locale extension, or `None` if not present.
    pub lang: Option<LanguageIdentifier>,
    /// The key-value pairs present in this locale extension, with each extension key subtag
    /// associated to its provided value subtag.
    pub fields: Fields,
}

impl Transform {
    /// Returns a new empty map of Transform extensions. Same as [`default()`](Default::default()), but is `const`.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::transform::Transform;
    ///
    /// assert_eq!(Transform::new(), Transform::default());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            lang: None,
            fields: Fields::new(),
        }
    }

    /// A constructor which takes a str slice, parses it and
    /// produces a well-formed [`Transform`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn try_from_str(s: &str) -> Result<Self, ParseError> {
        Self::try_from_utf8(s.as_bytes())
    }

    /// See [`Self::try_from_str`]
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn try_from_utf8(code_units: &[u8]) -> Result<Self, ParseError> {
        let mut iter = SubtagIterator::new(code_units);

        let ext = iter.next().ok_or(ParseError::InvalidExtension)?;
        if let ExtensionType::Transform = ExtensionType::try_from_byte_slice(ext)? {
            return Self::try_from_iter(&mut iter);
        }

        Err(ParseError::InvalidExtension)
    }

    /// Returns `true` if there are no tfields and no tlang in the `TransformExtensionList`.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    ///
    /// let mut loc: Locale = "en-US-t-es-ar".parse().expect("Parsing failed.");
    ///
    /// assert!(!loc.extensions.transform.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.lang.is_none() && self.fields.is_empty()
    }

    /// Clears the transform extension, effectively removing it from the locale.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    ///
    /// let mut loc: Locale = "en-US-t-es-ar".parse().unwrap();
    /// loc.extensions.transform.clear();
    /// assert_eq!(loc, "en-US".parse().unwrap());
    /// ```
    pub fn clear(&mut self) {
        self.lang = None;
        self.fields.clear();
    }

    #[expect(clippy::type_complexity)]
    pub(crate) fn as_tuple(
        &self,
    ) -> (
        Option<(
            subtags::Language,
            Option<subtags::Script>,
            Option<subtags::Region>,
            &subtags::Variants,
        )>,
        &Fields,
    ) {
        (self.lang.as_ref().map(|l| l.as_tuple()), &self.fields)
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

    #[cfg(feature = "alloc")]
    pub(crate) fn try_from_iter(iter: &mut SubtagIterator) -> Result<Self, ParseError> {
        let mut tlang = None;
        let mut tfields = LiteMap::new();

        if let Some(subtag) = iter.peek() {
            if Language::try_from_utf8(subtag).is_ok() {
                tlang = Some(parse_language_identifier_from_iter(
                    iter,
                    ParserMode::Partial,
                )?);
            }
        }

        let mut current_tkey = None;
        let mut current_tvalue = ShortBoxSlice::new();
        let mut has_current_tvalue = false;

        while let Some(subtag) = iter.peek() {
            if let Some(tkey) = current_tkey {
                if let Ok(val) = Value::parse_subtag(subtag) {
                    has_current_tvalue = true;
                    if let Some(val) = val {
                        current_tvalue.push(val);
                    }
                } else {
                    if !has_current_tvalue {
                        return Err(ParseError::InvalidExtension);
                    }
                    tfields.try_insert(tkey, Value::from_short_slice_unchecked(current_tvalue));
                    current_tkey = None;
                    current_tvalue = ShortBoxSlice::new();
                    has_current_tvalue = false;
                    continue;
                }
            } else if let Ok(tkey) = Key::try_from_utf8(subtag) {
                current_tkey = Some(tkey);
            } else {
                break;
            }

            iter.next();
        }

        if let Some(tkey) = current_tkey {
            if !has_current_tvalue {
                return Err(ParseError::InvalidExtension);
            }
            tfields.try_insert(tkey, Value::from_short_slice_unchecked(current_tvalue));
        }

        if tlang.is_none() && tfields.is_empty() {
            Err(ParseError::InvalidExtension)
        } else {
            Ok(Self {
                lang: tlang,
                fields: tfields.into(),
            })
        }
    }

    pub(crate) fn for_each_subtag_str<E, F>(&self, f: &mut F, with_ext: bool) -> Result<(), E>
    where
        F: FnMut(&str) -> Result<(), E>,
    {
        if self.is_empty() {
            return Ok(());
        }
        if with_ext {
            f(TRANSFORM_EXT_STR)?;
        }
        if let Some(lang) = &self.lang {
            lang.for_each_subtag_str_lowercased(f)?;
        }
        self.fields.for_each_subtag_str(f)
    }
}

/// ✨ *Enabled with the `alloc` Cargo feature.*
#[cfg(feature = "alloc")]
impl FromStr for Transform {
    type Err = ParseError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_str(s)
    }
}

writeable::impl_display_with_writeable!(Transform, #[cfg(feature = "alloc")]);

impl writeable::Writeable for Transform {
    fn write_to<W: core::fmt::Write + ?Sized>(&self, sink: &mut W) -> core::fmt::Result {
        if self.is_empty() {
            return Ok(());
        }
        sink.write_char(TRANSFORM_EXT_CHAR)?;
        if let Some(lang) = &self.lang {
            sink.write_char('-')?;
            lang.write_lowercased_to(sink)?;
        }
        if !self.fields.is_empty() {
            sink.write_char('-')?;
            writeable::Writeable::write_to(&self.fields, sink)?;
        }
        Ok(())
    }

    fn writeable_length_hint(&self) -> writeable::LengthHint {
        if self.is_empty() {
            return writeable::LengthHint::exact(0);
        }
        let mut result = writeable::LengthHint::exact(1);
        if let Some(lang) = &self.lang {
            result += writeable::Writeable::writeable_length_hint(lang) + 1;
        }
        if !self.fields.is_empty() {
            result += writeable::Writeable::writeable_length_hint(&self.fields) + 1;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_extension_fromstr() {
        let te: Transform = "t-en-us-h0-hybrid"
            .parse()
            .expect("Failed to parse Transform");
        assert_eq!(te.to_string(), "t-en-us-h0-hybrid");

        let te: Result<Transform, _> = "t".parse();
        assert!(te.is_err());
    }
}
