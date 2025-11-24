// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::preferences::extensions::unicode::enum_keyword;

enum_keyword!(
    /// A Unicode Currency Format Identifier defines a style for currency formatting.
    ///
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#UnicodeCurrencyFormatIdentifier).
    [Default]
    CurrencyFormatStyle {
        /// Negative numbers use the minusSign symbol (the default)
        [default]
        ("standard" => Standard),
        /// Negative numbers use parentheses or equivalent
        ("account" => Account)
}, "cf");
