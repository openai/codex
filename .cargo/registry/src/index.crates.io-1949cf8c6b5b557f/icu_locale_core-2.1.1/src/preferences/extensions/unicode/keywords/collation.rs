// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::preferences::extensions::unicode::enum_keyword;

enum_keyword!(
    /// A Unicode Collation Identifier defines a type of collation (sort order).
    ///
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#UnicodeCollationIdentifier).
    CollationType {
        /// A previous version of the ordering, for compatibility
        ("compat" => Compat),
        /// Dictionary style ordering (such as in Sinhala)
        ("dict" => Dict),
        /// The default Unicode collation element table order
        ("ducet" => Ducet),
        /// Recommended ordering for emoji characters
        ("emoji" => Emoji),
        /// European ordering rules
        ("eor" => Eor),
        /// Phonebook style ordering (such as in German)
        ("phonebk" => Phonebk),
        /// Phonetic ordering (sorting based on pronunciation)
        ("phonetic" => Phonetic),
        /// Pinyin ordering for Latin and for CJK characters (used in Chinese)
        ("pinyin" => Pinyin),
        /// Special collation type for string search
        ("search" => Search),
        /// Special collation type for Korean initial consonant search
        ("searchjl" => Searchjl),
        /// Default ordering for each language
        ("standard" => Standard),
        /// Pinyin ordering for Latin, stroke order for CJK characters (used in Chinese)
        ("stroke" => Stroke),
        /// Traditional style ordering (such as in Spanish)
        ("trad" => Trad),
        /// Pinyin ordering for Latin, Unihan radical-stroke ordering for CJK characters (used in Chinese)
        ("unihan" => Unihan),
        /// Pinyin ordering for Latin, zhuyin order for Bopomofo and CJK characters (used in Chinese)
        ("zhuyin" => Zhuyin),
}, "co");

enum_keyword!(
    /// Collation parameter key for ordering by case.
    ///
    /// If set to upper, causes upper case to sort before lower case. If set to lower, causes lower case to sort before upper case.
    /// Useful for locales that have already supported ordering but require different order of cases. Affects case and tertiary levels.
    ///
    /// The defails see [LDML](https://unicode.org/reports/tr35/tr35-collation.html#Case_Parameters).
    [Default]
    CollationCaseFirst {
        /// Upper case to be sorted before lower case
        ("upper" => Upper),
        /// Lower case to be sorted before upper case
        ("lower" => Lower),
        /// No special case ordering
        [default]
        ("false" => False),
}, "kf");

enum_keyword!(
    /// Collation parameter key for numeric handling.
    ///
    /// If set to on, any sequence of Decimal Digits (General_Category = Nd in the UAX44) is sorted at a primary level with
    /// its numeric value. For example, "1" < "2" < "10". The computed primary weights are all at the start of the digit
    /// reordering group.
    [Default]
    CollationNumericOrdering {
        /// A sequence of decimal digits is sorted at primary level with its numeric value
        ("true" => True),
        /// No special handling for numeric ordering
        [default]
        ("false" => False),
}, "kn");
