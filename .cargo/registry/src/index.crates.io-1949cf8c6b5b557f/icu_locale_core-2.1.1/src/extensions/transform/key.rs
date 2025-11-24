// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

impl_tinystr_subtag!(
    /// A key used in a list of [`Fields`](super::Fields).
    ///
    /// The key has to be a two ASCII characters long, with the first
    /// character being alphabetic, and the second being a number.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::transform::Key;
    ///
    /// let key1: Key = "k0".parse().expect("Failed to parse a Key.");
    ///
    /// assert_eq!(key1.as_str(), "k0");
    /// ```
    Key,
    extensions::transform,
    key,
    extensions_transform_key,
    2..=2,
    s,
    s.all_bytes()[0].is_ascii_alphabetic() && s.all_bytes()[1].is_ascii_digit(),
    s.to_ascii_lowercase(),
    s.all_bytes()[0].is_ascii_lowercase() && s.all_bytes()[1].is_ascii_digit(),
    InvalidExtension,
    ["k0"],
    ["", "k", "0k", "k12"],
);
