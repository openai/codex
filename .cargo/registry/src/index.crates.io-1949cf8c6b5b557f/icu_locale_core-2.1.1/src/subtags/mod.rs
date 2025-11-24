// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Language Identifier and Locale contains a set of subtags
//! which represent different fields of the structure.
//!
//! * [`Language`] is the only mandatory field, which when empty,
//!   takes the value `und`.
//! * [`Script`] is an optional field representing the written script used by the locale.
//! * [`Region`] is the region used by the locale.
//! * [`Variants`] is a list of optional [`Variant`] subtags containing information about the
//!   variant adjustments used by the locale.
//!
//! Subtags can be used in isolation, and all basic operations such as parsing, syntax normalization
//! and serialization are supported on each individual subtag, but most commonly
//! they are used to construct a [`LanguageIdentifier`] instance.
//!
//! [`Variants`] is a special structure which contains a list of [`Variant`] subtags.
//! It is wrapped around to allow for sorting and deduplication of variants, which
//! is one of the required steps of language identifier and locale syntax normalization.
//!
//! # Examples
//!
//! ```
//! use icu::locale::subtags::{Language, Region, Script, Variant};
//!
//! let language: Language =
//!     "en".parse().expect("Failed to parse a language subtag.");
//! let script: Script =
//!     "arab".parse().expect("Failed to parse a script subtag.");
//! let region: Region =
//!     "cn".parse().expect("Failed to parse a region subtag.");
//! let variant: Variant =
//!     "MacOS".parse().expect("Failed to parse a variant subtag.");
//!
//! assert_eq!(language.as_str(), "en");
//! assert_eq!(script.as_str(), "Arab");
//! assert_eq!(region.as_str(), "CN");
//! assert_eq!(variant.as_str(), "macos");
//! ```
//!
//! `Notice`: The subtags are normalized on parsing. That means
//! that all operations work on a normalized version of the subtag
//! and serialization is very cheap.
//!
//! [`LanguageIdentifier`]: super::LanguageIdentifier
mod language;
mod region;
mod script;
mod variant;
mod variants;

#[doc(inline)]
pub use language::{language, Language};
#[doc(inline)]
pub use region::{region, Region};
#[doc(inline)]
pub use script::{script, Script};
#[doc(inline)]
pub use variant::{variant, Variant};
pub use variants::Variants;

impl_tinystr_subtag!(
    /// A generic subtag.
    ///
    /// The subtag has to be an ASCII alphanumerical string no shorter than
    /// two characters and no longer than eight.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::subtags::Subtag;
    ///
    /// let subtag1: Subtag = "Foo".parse()
    ///     .expect("Failed to parse a Subtag.");
    ///
    /// assert_eq!(subtag1.as_str(), "foo");
    /// ```
    Subtag,
    subtags,
    subtag,
    subtags_subtag,
    2..=8,
    s,
    s.is_ascii_alphanumeric(),
    s.to_ascii_lowercase(),
    s.is_ascii_alphanumeric() && s.is_ascii_lowercase(),
    InvalidSubtag,
    ["foo12"],
    ["f", "toolooong"],
);

#[expect(clippy::len_without_is_empty)]
impl Subtag {
    #[allow(dead_code)]
    pub(crate) const fn valid_key(v: &[u8]) -> bool {
        2 <= v.len() && v.len() <= 8
    }

    /// Returns the length of `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::subtags::subtag;
    /// let s = subtag!("foo");
    /// assert_eq!(s.len(), 3);
    /// ```
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[doc(hidden)]
    pub fn from_tinystr_unvalidated(input: tinystr::TinyAsciiStr<8>) -> Self {
        Self(input)
    }

    #[doc(hidden)]
    pub fn as_tinystr(&self) -> tinystr::TinyAsciiStr<8> {
        self.0
    }

    #[allow(dead_code)]
    pub(crate) fn to_ascii_lowercase(self) -> Self {
        Self(self.0.to_ascii_lowercase())
    }
}

impl<const N: usize> TryFrom<tinystr::TinyAsciiStr<N>> for Subtag {
    type Error = crate::parser::errors::ParseError;

    fn try_from(value: tinystr::TinyAsciiStr<N>) -> Result<Self, Self::Error> {
        Self::try_from_str(&value)
    }
}

impl PartialEq<str> for Subtag {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinystr::tinystr;

    #[test]
    fn test_subtag() {
        let subtag = subtag!("foo");
        assert_eq!(subtag.as_str(), "foo");
    }

    #[test]
    fn test_subtag_from_tinystr() {
        let subtag = Subtag::try_from(tinystr!(3, "foo"));
        assert!(subtag.is_ok());

        let subtag = Subtag::try_from(tinystr!(1, "f"));
        assert!(subtag.is_err());
    }
}
