// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use displaydoc::Display;

/// List of parser errors that can be generated
/// while parsing [`LanguageIdentifier`](crate::LanguageIdentifier), [`Locale`](crate::Locale),
/// [`subtags`](crate::subtags) or [`extensions`](crate::extensions).
#[derive(Display, Debug, PartialEq, Copy, Clone)]
#[non_exhaustive]
pub enum ParseError {
    /// Invalid language subtag.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::subtags::Language;
    /// use icu::locale::ParseError;
    ///
    /// assert_eq!("x2".parse::<Language>(), Err(ParseError::InvalidLanguage));
    /// ```
    #[displaydoc("The given language subtag is invalid")]
    InvalidLanguage,

    /// Invalid script, region or variant subtag.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::subtags::Region;
    /// use icu::locale::ParseError;
    ///
    /// assert_eq!("#@2X".parse::<Region>(), Err(ParseError::InvalidSubtag));
    /// ```
    #[displaydoc("Invalid subtag")]
    InvalidSubtag,

    /// Invalid extension subtag.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::Key;
    /// use icu::locale::ParseError;
    ///
    /// assert_eq!("#@2X".parse::<Key>(), Err(ParseError::InvalidExtension));
    /// ```
    #[displaydoc("Invalid extension")]
    InvalidExtension,

    /// Duplicated extension.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    /// use icu::locale::ParseError;
    ///
    /// assert_eq!(
    ///     "und-u-hc-h12-u-ca-calendar".parse::<Locale>(),
    ///     Err(ParseError::DuplicatedExtension)
    /// );
    /// ```
    #[displaydoc("Duplicated extension")]
    DuplicatedExtension,
}

impl core::error::Error for ParseError {}
