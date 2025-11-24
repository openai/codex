// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::subtags::Subtag;

impl_tinystr_subtag!(
    /// A script subtag (examples: `"Latn"`, `"Arab"`, etc.)
    ///
    /// [`Script`] represents a Unicode base language code conformant to the
    /// [`unicode_script_id`] field of the Language and Locale Identifier.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::subtags::Script;
    ///
    /// let script: Script =
    ///     "Latn".parse().expect("Failed to parse a script subtag.");
    /// ```
    ///
    /// [`unicode_script_id`]: https://unicode.org/reports/tr35/#unicode_script_id
    Script,
    subtags,
    script,
    subtags_script,
    4..=4,
    s,
    s.is_ascii_alphabetic(),
    s.to_ascii_titlecase(),
    s.is_ascii_alphabetic_titlecase(),
    InvalidSubtag,
    ["Latn"],
    ["Latin"],
);

impl From<Script> for Subtag {
    fn from(value: Script) -> Self {
        Subtag(value.0.resize())
    }
}
