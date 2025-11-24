// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::preferences::extensions::unicode::errors::PreferencesParseError;
use crate::preferences::extensions::unicode::struct_keyword;
use crate::{extensions::unicode::Value, subtags::Subtag};

struct_keyword!(
    /// A Unicode Number System Identifier defines a type of number system.
    ///
    /// The valid values are listed in [LDML](https://unicode.org/reports/tr35/#UnicodeNumberSystemIdentifier).
    [Copy]
    NumberingSystem,
    "nu",
    Subtag,
    |input: Value| {
        input
            .into_single_subtag()
            .map(Self)
            .ok_or(PreferencesParseError::InvalidKeywordValue)
    },
    |input: NumberingSystem| {
        crate::extensions::unicode::Value::from_subtag(Some(input.0))
    }
);
