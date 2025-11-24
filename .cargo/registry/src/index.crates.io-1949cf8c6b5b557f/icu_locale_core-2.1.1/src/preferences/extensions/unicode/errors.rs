// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Errors related to parsing of Preferences.

/// Error returned by parsers of unicode extensions as preferences.
#[non_exhaustive]
#[derive(Debug, displaydoc::Display)]
pub enum PreferencesParseError {
    /// The given keyword value is not a valid preference variant.
    InvalidKeywordValue,
}

impl core::error::Error for PreferencesParseError {}
