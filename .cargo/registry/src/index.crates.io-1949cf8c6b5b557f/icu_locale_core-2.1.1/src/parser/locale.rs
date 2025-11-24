// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::extensions;
use crate::parser::{ParseError, ParserMode, SubtagIterator};
use crate::subtags::{self, Subtag};
#[cfg(feature = "alloc")]
use crate::Locale;

use super::parse_locale_with_single_variant_single_keyword_unicode_extension_from_iter;

#[cfg(feature = "alloc")]
pub fn parse_locale(t: &[u8]) -> Result<Locale, ParseError> {
    let mut iter = SubtagIterator::new(t);

    let id = super::parse_language_identifier_from_iter(&mut iter, ParserMode::Locale)?;
    let extensions = if iter.peek().is_some() {
        extensions::Extensions::try_from_iter(&mut iter)?
    } else {
        extensions::Extensions::default()
    };
    Ok(Locale { id, extensions })
}

#[expect(clippy::type_complexity)]
pub const fn parse_locale_with_single_variant_single_keyword_unicode_keyword_extension(
    t: &[u8],
    mode: ParserMode,
) -> Result<
    (
        subtags::Language,
        Option<subtags::Script>,
        Option<subtags::Region>,
        Option<subtags::Variant>,
        Option<(extensions::unicode::Key, Option<Subtag>)>,
    ),
    ParseError,
> {
    let iter = SubtagIterator::new(t);
    parse_locale_with_single_variant_single_keyword_unicode_extension_from_iter(iter, mode)
}
