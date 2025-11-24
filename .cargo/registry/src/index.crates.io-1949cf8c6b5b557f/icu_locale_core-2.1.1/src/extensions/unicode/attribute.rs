// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

impl_tinystr_subtag!(
    /// An attribute used in a set of [`Attributes`](super::Attributes).
    ///
    /// An attribute has to be a sequence of alphanumerical characters no
    /// shorter than three and no longer than eight characters.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::{attribute, Attribute};
    ///
    /// let attr: Attribute =
    ///     "buddhist".parse().expect("Failed to parse an Attribute.");
    ///
    /// assert_eq!(attr, attribute!("buddhist"));
    /// ```
    Attribute,
    extensions::unicode,
    attribute,
    extensions_unicode_attribute,
    3..=8,
    s,
    s.is_ascii_alphanumeric(),
    s.to_ascii_lowercase(),
    s.is_ascii_alphanumeric() && s.is_ascii_lowercase(),
    InvalidExtension,
    ["foo12"],
    ["no", "toolooong"],
);
