// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::preferences::extensions::unicode::enum_keyword;

enum_keyword!(
    /// A Unicode Line Break Style Identifier defines a preferred line break style corresponding to the CSS level 3 line-break option.
    ///
    /// Specifying "lb" in a locale identifier overrides the locale’s default style
    /// (which may correspond to "normal" or "strict").
    ///
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#UnicodeLineBreakStyleIdentifier).
    LineBreakStyle {
        /// CSS level 3 line-break=strict, e.g. treat CJ as NS
        ("strict" => Strict),
        /// CSS level 3 line-break=normal, e.g. treat CJ as ID, break before hyphens for ja,zh
        ("normal" => Normal),
        /// CSS lev 3 line-break=loose
        ("loose" => Loose),
}, "lb");
