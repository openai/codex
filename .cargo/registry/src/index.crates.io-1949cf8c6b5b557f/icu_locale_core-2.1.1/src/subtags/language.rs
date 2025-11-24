// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

impl_tinystr_subtag!(
    /// A language subtag (examples: `"en"`, `"csb"`, `"zh"`, `"und"`, etc.)
    ///
    /// [`Language`] represents a Unicode base language code conformant to the
    /// [`unicode_language_id`] field of the Language and Locale Identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::subtags::Language;
    ///
    /// let language: Language =
    ///     "en".parse().expect("Failed to parse a language subtag.");
    /// ```
    ///
    /// If the [`Language`] has no value assigned, it serializes to a string `"und"`, which
    /// can be then parsed back to an empty [`Language`] field.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::subtags::Language;
    ///
    /// assert_eq!(Language::UNKNOWN.as_str(), "und");
    /// ```
    ///
    /// `Notice`: ICU4X uses a narrow form of language subtag of 2-3 characters.
    /// The specification allows language subtag to optionally also be 5-8 characters
    /// but that form has not been used and ICU4X does not support it right now.
    ///
    /// [`unicode_language_id`]: https://unicode.org/reports/tr35/#unicode_language_id
    Language,
    subtags,
    language,
    subtags_language,
    2..=3,
    s,
    s.is_ascii_alphabetic(),
    s.to_ascii_lowercase(),
    s.is_ascii_alphabetic_lowercase(),
    InvalidLanguage,
    ["en", "foo"],
    ["419", "german", "en1"],
);

impl Language {
    /// The unknown language "und".
    pub const UNKNOWN: Self = language!("und");

    /// Whether this [`Language`] equals [`Language::UNKNOWN`].
    #[inline]
    pub const fn is_unknown(self) -> bool {
        matches!(self, Self::UNKNOWN)
    }
}
