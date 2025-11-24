// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

/// A macro allowing for compile-time construction of valid [`LanguageIdentifier`]s.
///
/// The macro will perform syntax normalization of the tag.
///
/// # Examples
///
/// ```
/// use icu::locale::{langid, LanguageIdentifier};
///
/// const DE_AT: LanguageIdentifier = langid!("de-at");
///
/// let de_at: LanguageIdentifier = "de-at".parse().unwrap();
///
/// assert_eq!(DE_AT, de_at);
/// ```
///
/// *Note*: The macro cannot produce language identifiers with more than one variants due to const
/// limitations (see [`Heap Allocations in Constants`]):
///
/// ```compile_fail,E0080
/// icu::locale::langid!("und-variant1-variant2");
/// ```
///
/// Use runtime parsing instead:
/// ```
/// "und-variant1-variant2"
///     .parse::<icu::locale::LanguageIdentifier>()
///     .unwrap();
/// ```
///
/// [`LanguageIdentifier`]: crate::LanguageIdentifier
/// [`Heap Allocations in Constants`]: https://github.com/rust-lang/const-eval/issues/20
#[macro_export]
macro_rules! langid {
    ($langid:literal) => { const {
        match $crate::LanguageIdentifier::try_from_utf8_with_single_variant($langid.as_bytes()) {
            Ok((language, script, region, variant)) => $crate::LanguageIdentifier {
                language,
                script,
                region,
                variants: match variant {
                    Some(v) => $crate::subtags::Variants::from_variant(v),
                    None => $crate::subtags::Variants::new(),
                }
            },
            _ => panic!(concat!("Invalid language code: ", $langid, " . Note langid! macro can only support up to a single variant tag. Use runtime parsing instead.")),
        }
    }};
}

/// A macro allowing for compile-time construction of valid [`Locale`]s.
///
/// The macro will perform syntax normalization of the tag.
///
/// # Examples
///
/// ```
/// use icu::locale::{locale, Locale};
///
/// const DE_AT: Locale = locale!("de-at");
///
/// let de_at: Locale = "de-at".parse().unwrap();
///
/// assert_eq!(DE_AT, de_at);
/// ```
///
/// *Note*: The macro cannot produce locales with more than one variant or multiple extensions
/// (only single keyword unicode extension is supported) due to const
/// limitations (see [`Heap Allocations in Constants`]):
///
/// ```compile_fail,E0080
/// icu::locale::locale!("sl-IT-rozaj-biske-1994");
/// ```
/// Use runtime parsing instead:
/// ```
/// "sl-IT-rozaj-biske-1994"
///     .parse::<icu::locale::Locale>()
///     .unwrap();
/// ```
///
/// Locales with multiple keys are not supported
/// ```compile_fail,E0080
/// icu::locale::locale!("th-TH-u-ca-buddhist-nu-thai");
/// ```
/// Use runtime parsing instead:
/// ```
/// "th-TH-u-ca-buddhist-nu-thai"
///     .parse::<icu::locale::Locale>()
///     .unwrap();
/// ```
///
/// Locales with attributes are not supported
/// ```compile_fail,E0080
/// icu::locale::locale!("en-US-u-foobar-ca-buddhist");
/// ```
/// Use runtime parsing instead:
/// ```
/// "en-US-u-foobar-ca-buddhist"
///     .parse::<icu::locale::Locale>()
///     .unwrap();
/// ```
///
/// Locales with single key but multiple types are not supported
/// ```compile_fail,E0080
/// icu::locale::locale!("en-US-u-ca-islamic-umalqura");
/// ```
/// Use runtime parsing instead:
/// ```
/// "en-US-u-ca-islamic-umalqura"
///     .parse::<icu::locale::Locale>()
///     .unwrap();
/// ```
/// [`Locale`]: crate::Locale
/// [`Heap Allocations in Constants`]: https://github.com/rust-lang/const-eval/issues/20
#[macro_export]
macro_rules! locale {
    ($locale:literal) => { const {
        match $crate::Locale::try_from_utf8_with_single_variant_single_keyword_unicode_extension(
            $locale.as_bytes(),
        ) {
            Ok((language, script, region, variant, keyword)) => $crate::Locale {
                id: $crate::LanguageIdentifier {
                    language,
                    script,
                    region,
                    variants: match variant {
                        Some(v) => $crate::subtags::Variants::from_variant(v),
                        None => $crate::subtags::Variants::new(),
                    },
                },
                extensions: match keyword {
                    Some(k) => $crate::extensions::Extensions::from_unicode(
                        $crate::extensions::unicode::Unicode {
                            keywords: $crate::extensions::unicode::Keywords::new_single(
                                k.0,
                                $crate::extensions::unicode::Value::from_subtag(k.1),
                            ),

                            attributes: $crate::extensions::unicode::Attributes::new(),
                        },
                    ),
                    None => $crate::extensions::Extensions::new(),
                },
            },
            _ => panic!(concat!(
                "Invalid language code: ",
                $locale,
                " . Note the locale! macro only supports up to one variant tag; \
                                    and one unicode keyword, other extension are \
                                    not supported. Use runtime parsing instead."
            )),
        }
    }};
}

#[cfg(test)]
mod test {
    use crate::LanguageIdentifier;
    use crate::Locale;

    #[test]
    fn test_langid_macro_can_parse_langid_with_single_variant() {
        const DE_AT_FOOBAR: LanguageIdentifier = langid!("de-at-foobar");
        let de_at_foobar: LanguageIdentifier = "de-at-foobar".parse().unwrap();
        assert_eq!(DE_AT_FOOBAR, de_at_foobar);
    }

    #[test]
    fn test_locale_macro_can_parse_locale_with_single_variant() {
        const DE_AT_FOOBAR: Locale = locale!("de-at-foobar");
        let de_at_foobar: Locale = "de-at-foobar".parse().unwrap();
        assert_eq!(DE_AT_FOOBAR, de_at_foobar);
    }

    #[test]
    fn test_locale_macro_can_parse_locale_with_single_keyword_unicode_extension() {
        const DE_AT_U_CA_FOOBAR: Locale = locale!("de-at-u-ca-foobar");
        let de_at_u_ca_foobar: Locale = "de-at-u-ca-foobar".parse().unwrap();
        assert_eq!(DE_AT_U_CA_FOOBAR, de_at_u_ca_foobar);
    }
}
