// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::preferences::extensions::unicode::enum_keyword;

enum_keyword!(
    /// A Unicode Variant Identifier defines a special variant used for locales.
    ///
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#UnicodeVariantIdentifier).
    CommonVariantType {
        /// POSIX style locale variant
        ("posix" => Posix),
}, "va");
