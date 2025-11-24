// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

pub use super::errors::ParseError;
use crate::extensions::unicode::{Attribute, Key, Value};
use crate::extensions::ExtensionType;
use crate::parser::SubtagIterator;
#[cfg(feature = "alloc")]
use crate::shortvec::ShortBoxSlice;
use crate::subtags::Subtag;
#[cfg(feature = "alloc")]
use crate::LanguageIdentifier;
use crate::{extensions, subtags};

#[derive(PartialEq, Clone, Copy)]
pub enum ParserMode {
    LanguageIdentifier,
    Locale,
    #[allow(dead_code)]
    Partial,
}

#[derive(PartialEq, Clone, Copy)]
enum ParserPosition {
    Script,
    Region,
    Variant,
}

#[cfg(feature = "alloc")]
pub fn parse_language_identifier_from_iter(
    iter: &mut SubtagIterator,
    mode: ParserMode,
) -> Result<LanguageIdentifier, ParseError> {
    let mut script = None;
    let mut region = None;
    let mut variants = ShortBoxSlice::new();

    let language = if let Some(subtag) = iter.next() {
        subtags::Language::try_from_utf8(subtag)?
    } else {
        return Err(ParseError::InvalidLanguage);
    };

    let mut position = ParserPosition::Script;

    while let Some(subtag) = iter.peek() {
        if mode != ParserMode::LanguageIdentifier && subtag.len() == 1 {
            break;
        }

        if position == ParserPosition::Script {
            if let Ok(s) = subtags::Script::try_from_utf8(subtag) {
                script = Some(s);
                position = ParserPosition::Region;
            } else if let Ok(s) = subtags::Region::try_from_utf8(subtag) {
                region = Some(s);
                position = ParserPosition::Variant;
            } else if let Ok(v) = subtags::Variant::try_from_utf8(subtag) {
                if let Err(idx) = variants.binary_search(&v) {
                    variants.insert(idx, v);
                }
                position = ParserPosition::Variant;
            } else if mode == ParserMode::Partial {
                break;
            } else {
                return Err(ParseError::InvalidSubtag);
            }
        } else if position == ParserPosition::Region {
            if let Ok(s) = subtags::Region::try_from_utf8(subtag) {
                region = Some(s);
                position = ParserPosition::Variant;
            } else if let Ok(v) = subtags::Variant::try_from_utf8(subtag) {
                if let Err(idx) = variants.binary_search(&v) {
                    variants.insert(idx, v);
                }
                position = ParserPosition::Variant;
            } else if mode == ParserMode::Partial {
                break;
            } else {
                return Err(ParseError::InvalidSubtag);
            }
        } else if let Ok(v) = subtags::Variant::try_from_utf8(subtag) {
            if let Err(idx) = variants.binary_search(&v) {
                variants.insert(idx, v);
            } else {
                return Err(ParseError::InvalidSubtag);
            }
        } else if mode == ParserMode::Partial {
            break;
        } else {
            return Err(ParseError::InvalidSubtag);
        }
        iter.next();
    }

    Ok(LanguageIdentifier {
        language,
        script,
        region,
        variants: subtags::Variants::from_short_slice_unchecked(variants),
    })
}

#[cfg(feature = "alloc")]
pub fn parse_language_identifier(
    t: &[u8],
    mode: ParserMode,
) -> Result<LanguageIdentifier, ParseError> {
    let mut iter = SubtagIterator::new(t);
    parse_language_identifier_from_iter(&mut iter, mode)
}

#[expect(clippy::type_complexity)]
pub const fn parse_locale_with_single_variant_single_keyword_unicode_extension_from_iter(
    mut iter: SubtagIterator,
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
    let language;
    let mut script = None;
    let mut region = None;
    let mut variant = None;
    let mut keyword = None;

    if let (i, Some(subtag)) = iter.next_const() {
        iter = i;
        match subtags::Language::try_from_utf8(subtag) {
            Ok(l) => language = l,
            Err(e) => return Err(e),
        }
    } else {
        return Err(ParseError::InvalidLanguage);
    }

    let mut position = ParserPosition::Script;

    while let Some(subtag) = iter.peek() {
        if !matches!(mode, ParserMode::LanguageIdentifier) && subtag.len() == 1 {
            break;
        }

        if matches!(position, ParserPosition::Script) {
            if let Ok(s) = subtags::Script::try_from_utf8(subtag) {
                script = Some(s);
                position = ParserPosition::Region;
            } else if let Ok(r) = subtags::Region::try_from_utf8(subtag) {
                region = Some(r);
                position = ParserPosition::Variant;
            } else if let Ok(v) = subtags::Variant::try_from_utf8(subtag) {
                // We cannot handle multiple variants in a const context
                debug_assert!(variant.is_none());
                variant = Some(v);
                position = ParserPosition::Variant;
            } else if matches!(mode, ParserMode::Partial) {
                break;
            } else {
                return Err(ParseError::InvalidSubtag);
            }
        } else if matches!(position, ParserPosition::Region) {
            if let Ok(s) = subtags::Region::try_from_utf8(subtag) {
                region = Some(s);
                position = ParserPosition::Variant;
            } else if let Ok(v) = subtags::Variant::try_from_utf8(subtag) {
                // We cannot handle multiple variants in a const context
                debug_assert!(variant.is_none());
                variant = Some(v);
                position = ParserPosition::Variant;
            } else if matches!(mode, ParserMode::Partial) {
                break;
            } else {
                return Err(ParseError::InvalidSubtag);
            }
        } else if let Ok(v) = subtags::Variant::try_from_utf8(subtag) {
            debug_assert!(matches!(position, ParserPosition::Variant));
            if variant.is_some() {
                // We cannot handle multiple variants in a const context
                return Err(ParseError::InvalidSubtag);
            }
            variant = Some(v);
        } else if matches!(mode, ParserMode::Partial) {
            break;
        } else {
            return Err(ParseError::InvalidSubtag);
        }

        iter = iter.next_const().0;
    }

    if matches!(mode, ParserMode::Locale) {
        if let Some(subtag) = iter.peek() {
            match ExtensionType::try_from_utf8(subtag) {
                Ok(ExtensionType::Unicode) => {
                    iter = iter.next_const().0;
                    if let Some(peek) = iter.peek() {
                        if Attribute::try_from_utf8(peek).is_ok() {
                            // We cannot handle Attributes in a const context
                            return Err(ParseError::InvalidSubtag);
                        }
                    }

                    let mut key = None;
                    let mut current_type = None;

                    while let Some(peek) = iter.peek() {
                        if peek.len() == 2 {
                            if key.is_some() {
                                // We cannot handle more than one Key in a const context
                                return Err(ParseError::InvalidSubtag);
                            }
                            match Key::try_from_utf8(peek) {
                                Ok(k) => key = Some(k),
                                Err(e) => return Err(e),
                            };
                        } else if key.is_some() {
                            match Value::parse_subtag_from_utf8(peek) {
                                Ok(Some(t)) => {
                                    if current_type.is_some() {
                                        // We cannot handle more than one type in a const context
                                        return Err(ParseError::InvalidSubtag);
                                    }
                                    current_type = Some(t);
                                }
                                Ok(None) => {}
                                Err(e) => return Err(e),
                            }
                        } else {
                            break;
                        }
                        iter = iter.next_const().0;
                    }
                    if let Some(k) = key {
                        keyword = Some((k, current_type));
                    }
                }
                // We cannot handle Transform, Private, Other extensions in a const context
                Ok(_) => return Err(ParseError::InvalidSubtag),
                Err(e) => return Err(e),
            }
        }
    }

    Ok((language, script, region, variant, keyword))
}

#[expect(clippy::type_complexity)]
pub const fn parse_language_identifier_with_single_variant(
    t: &[u8],
    mode: ParserMode,
) -> Result<
    (
        subtags::Language,
        Option<subtags::Script>,
        Option<subtags::Region>,
        Option<subtags::Variant>,
    ),
    ParseError,
> {
    let iter = SubtagIterator::new(t);
    match parse_locale_with_single_variant_single_keyword_unicode_extension_from_iter(iter, mode) {
        Ok((l, s, r, v, _)) => Ok((l, s, r, v)),
        Err(e) => Err(e),
    }
}
