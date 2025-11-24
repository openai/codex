// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

impl_tinystr_subtag!(
    /// A variant subtag (examples: `"macos"`, `"posix"`, `"1996"` etc.)
    ///
    /// [`Variant`] represents a Unicode base language code conformant to the
    /// [`unicode_variant_id`] field of the Language and Locale Identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::subtags::Variant;
    ///
    /// let variant: Variant =
    ///     "macos".parse().expect("Failed to parse a variant subtag.");
    /// ```
    ///
    /// [`unicode_variant_id`]: https://unicode.org/reports/tr35/#unicode_variant_id
    Variant,
    subtags,
    variant,
    subtags_variant,
    4..=8,
    s,
    s.is_ascii_alphanumeric() && (s.len() != 4 || s.all_bytes()[0].is_ascii_digit()),
    s.to_ascii_lowercase(),
    s.is_ascii_lowercase()
        && s.is_ascii_alphanumeric()
        && (s.len() != 4 || s.all_bytes()[0].is_ascii_digit()),
    InvalidSubtag,
    ["posix", "1996"],
    ["yes"],
);
