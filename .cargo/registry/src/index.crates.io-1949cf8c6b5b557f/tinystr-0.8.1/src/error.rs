// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use displaydoc::Display;

impl core::error::Error for ParseError {}

#[derive(Display, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseError {
    #[displaydoc("found string of larger length {len} when constructing string of length {max}")]
    TooLong { max: usize, len: usize },
    #[displaydoc("tinystr types do not support strings with null bytes")]
    ContainsNull,
    #[displaydoc("attempted to construct TinyAsciiStr from a non-ASCII string")]
    NonAscii,
}
