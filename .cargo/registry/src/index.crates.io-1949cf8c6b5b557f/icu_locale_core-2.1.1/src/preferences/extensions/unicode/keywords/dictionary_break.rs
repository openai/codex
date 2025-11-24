// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::extensions::unicode::Value;
use crate::preferences::extensions::unicode::errors::PreferencesParseError;
use crate::preferences::extensions::unicode::struct_keyword;
use crate::subtags::Script;
use alloc::vec::Vec;
use core::str::FromStr;

struct_keyword!(
    /// A Unicode Dictionary Break Exclusion Identifier specifies
    /// scripts to be excluded from dictionary-based text break (for words and lines).
    ///
    /// The valid values are of one or more items of type [`Script`](crate::subtags::Script).
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    DictionaryBreakScriptExclusions,
    "dx",
    Vec<Script>,
    |input: Value| {
        input
            .into_iter()
            .map(|s| {
                Script::from_str(s.as_str()).map_err(|_| PreferencesParseError::InvalidKeywordValue)
            })
            .collect::<Result<_, _>>()
            .map(Self)
    },
    |input: DictionaryBreakScriptExclusions| {
        input.0.into_iter().map(Into::into).collect()
    }
);
