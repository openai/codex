// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

impl_tinystr_subtag!(
    /// A single item used in a list of [`Private`](super::Private) extensions.
    ///
    /// The subtag has to be an ASCII alphanumerical string no shorter than
    /// one character and no longer than eight.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::private::Subtag;
    ///
    /// let subtag1: Subtag = "Foo".parse()
    ///     .expect("Failed to parse a Subtag.");
    ///
    /// assert_eq!(subtag1.as_str(), "foo");
    /// ```
    ///
    /// Notice: This is different from the generic [`Subtag`](crate::subtags::Subtag)
    /// which is between two and eight characters.
    ///
    /// ```
    /// use icu::locale::extensions::private;
    /// use icu::locale::subtags;
    ///
    /// let subtag: Result<private::Subtag, _> = "f".parse();
    /// assert!(subtag.is_ok());
    ///
    /// let subtag: Result<subtags::Subtag, _> = "f".parse();
    /// assert!(subtag.is_err());
    /// ```
    Subtag,
    extensions::private,
    subtag,
    extensions_private_subtag,
    1..=8,
    s,
    s.is_ascii_alphanumeric(),
    s.to_ascii_lowercase(),
    s.is_ascii_alphanumeric() && s.is_ascii_lowercase(),
    InvalidExtension,
    ["foo12"],
    ["toolooong"],
);
