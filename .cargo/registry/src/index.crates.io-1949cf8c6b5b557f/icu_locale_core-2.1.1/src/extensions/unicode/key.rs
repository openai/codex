// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

impl_tinystr_subtag!(
    /// A key used in a list of [`Keywords`](super::Keywords).
    ///
    /// The key has to be a two ASCII alphanumerical characters long, with the first
    /// character being alphanumeric, and the second being alphabetic.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::Key;
    ///
    /// assert!("ca".parse::<Key>().is_ok());
    /// ```
    Key,
    extensions::unicode,
    key,
    extensions_unicode_key,
    2..=2,
    s,
    s.all_bytes()[0].is_ascii_alphanumeric() && s.all_bytes()[1].is_ascii_alphabetic(),
    s.to_ascii_lowercase(),
    (s.all_bytes()[0].is_ascii_lowercase() || s.all_bytes()[0].is_ascii_digit())
        && s.all_bytes()[1].is_ascii_lowercase(),
    InvalidExtension,
    ["ca", "8a"],
    ["a", "a8", "abc"],
);
