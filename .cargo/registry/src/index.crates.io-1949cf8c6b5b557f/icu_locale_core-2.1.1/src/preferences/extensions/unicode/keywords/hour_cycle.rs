// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::preferences::extensions::unicode::enum_keyword;

enum_keyword!(
    /// A Unicode Hour Cycle Identifier defines the preferred time cycle. Specifying "hc" in a locale identifier overrides the default value specified by supplemental time data for the region.
    ///
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#UnicodeHourCycleIdentifier).
    HourCycle {
        /// The typical 12-hour clock. Hours are numbered 1–12. Corresponds to 'h' in patterns.
        ("h12" => H12),
        /// The 24-hour clock. Hour are numbered 0–23. Corresponds to 'H' in patterns.
        ("h23" => H23),
        /// Variant of the 12-hour clock, sometimes used in Japan. Hours are numbered 0–11. Corresponds to 'K' in patterns.
        ("h11" => H11),
}, "hc");
