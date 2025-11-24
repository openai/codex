// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::preferences::extensions::unicode::enum_keyword;

enum_keyword!(
    /// A Unicode First Day Identifier defines the preferred first day of the week for calendar display.
    ///
    /// Specifying "fw" in a locale identifier overrides the default value specified by
    /// supplemental week data for the region.
    ///
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#UnicodeFirstDayIdentifier).
    FirstDay {
        /// Sunday
        ("sun" => Sun),
        /// Monday
        ("mon" => Mon),
        /// Tuesday
        ("tue" => Tue),
        /// Wednesday
        ("wed" => Wed),
        /// Thursday
        ("thu" => Thu),
        /// Friday
        ("fri" => Fri),
        /// Saturday
        ("sat" => Sat)
}, "fw");
