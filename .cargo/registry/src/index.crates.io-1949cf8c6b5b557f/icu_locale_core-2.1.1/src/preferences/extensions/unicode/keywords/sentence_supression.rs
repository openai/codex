// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::preferences::extensions::unicode::enum_keyword;

enum_keyword!(
    /// A Unicode Sentence Break Suppressions Identifier defines a set of data to be used for suppressing certain
    /// sentence breaks that would otherwise be found by UAX #14 rules.
    ///
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#UnicodeSentenceBreakSuppressionsIdentifier).
    [Default]
    SentenceBreakSupressions {
        /// Don’t use sentence break suppressions data (the default)
        [default]
        ("none" => None),
        /// Use sentence break suppressions data of type "standard"
        ("standard" => Standard),
}, "ss");
