// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#![allow(non_snake_case)]

use crate::preferences::extensions::unicode::enum_keyword;

enum_keyword!(
    /// Hijri Calendar sub-type
    ///
    /// The list is based on [`CLDR Calendars`](https://github.com/unicode-org/cldr/blob/main/common/bcp47/calendar.xml)
    HijriCalendarAlgorithm {
        /// Hijri calendar, Umm al-Qura
        Umalqura,
        /// Hijri calendar, tabular (intercalary years \[2,5,7,10,13,16,18,21,24,26,29] - astronomical epoch)
        Tbla,
        /// Hijri calendar, tabular (intercalary years \[2,5,7,10,13,16,18,21,24,26,29] - civil epoch)
        Civil,
        /// Hijri calendar, Saudi Arabia sighting
        Rgsa
});

enum_keyword!(
    /// A Unicode Calendar Identifier defines a type of calendar.
    ///
    /// This selects calendar-specific data within a locale used for formatting and parsing,
    /// such as date/time symbols and patterns; it also selects supplemental calendarData used
    /// for calendrical calculations. The value can affect the computation of the first day of the week.
    ///
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#UnicodeCalendarIdentifier).
    CalendarAlgorithm {
        /// Thai Buddhist calendar (same as Gregorian except for the year)
        ("buddhist" => Buddhist),
        /// Traditional Chinese calendar
        ("chinese" => Chinese),
        /// Coptic calendar
        ("coptic" => Coptic),
        /// Traditional Korean calendar
        ("dangi" => Dangi),
        /// Ethiopic calendar, Amete Alem (epoch approx. 5493 B.C.E)
        ("ethioaa" => Ethioaa),
        /// Ethiopic calendar, Amete Mihret (epoch approx, 8 C.E.)
        ("ethiopic" => Ethiopic),
        /// Gregorian calendar
        ("gregory" => Gregory),
        /// Traditional Hebrew calendar
        ("hebrew" => Hebrew),
        /// Indian calendar
        ("indian" => Indian),
        /// Hijri calendar
        ("islamic" => Hijri(HijriCalendarAlgorithm) {
             ("umalqura" => Umalqura),
             ("tbla" => Tbla),
             ("civil" => Civil),
             ("rgsa" => Rgsa)
        }),
        /// ISO calendar (Gregorian calendar using the ISO 8601 calendar week rules)
        ("iso8601" => Iso8601),
        /// Japanese Imperial calendar
        ("japanese" => Japanese),
        /// Persian calendar
        ("persian" => Persian),
        /// Republic of China calendar
        ("roc" => Roc)
}, "ca", s, if *s == value!("islamicc") { return Ok(Self::Hijri(Some(HijriCalendarAlgorithm::Civil))); });
