// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! The collection of code for locale canonicalization.

use crate::provider::*;
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::LocaleExpander;
use crate::TransformResult;
use icu_locale_core::extensions::Extensions;
use icu_locale_core::subtags::{Language, Region, Script};
use icu_locale_core::{
    extensions::unicode::key,
    subtags::{language, Variant, Variants},
    LanguageIdentifier, Locale,
};
use icu_provider::prelude::*;
use tinystr::TinyAsciiStr;

/// Implements the algorithm defined in *[UTS #35: Annex C, LocaleId Canonicalization]*.
///
/// # Examples
///
/// ```
/// use icu::locale::Locale;
/// use icu::locale::{LocaleCanonicalizer, TransformResult};
///
/// let lc = LocaleCanonicalizer::new_extended();
///
/// let mut locale: Locale = "ja-Latn-fonipa-hepburn-heploc".parse().unwrap();
/// assert_eq!(lc.canonicalize(&mut locale), TransformResult::Modified);
/// assert_eq!(locale, "ja-Latn-alalc97-fonipa".parse().unwrap());
/// ```
///
/// [UTS #35: Annex C, LocaleId Canonicalization]: http://unicode.org/reports/tr35/#LocaleId_Canonicalization
#[derive(Debug)]
pub struct LocaleCanonicalizer<Expander = LocaleExpander> {
    /// Data to support canonicalization.
    aliases: DataPayload<LocaleAliasesV1>,
    /// Likely subtags implementation for delegation.
    expander: Expander,
}

fn uts35_rule_matches<'a, I>(
    source: &LanguageIdentifier,
    language: Language,
    script: Option<Script>,
    region: Option<Region>,
    raw_variants: I,
) -> bool
where
    I: Iterator<Item = &'a str>,
{
    (language.is_unknown() || language == source.language)
        && (script.is_none() || script == source.script)
        && (region.is_none() || region == source.region)
        && {
            // Checks if variants are a subset of source variants.
            // As both iterators are sorted, this can be done linearly.
            let mut source_variants = source.variants.iter();
            'outer: for raw_variant in raw_variants {
                for source_variant in source_variants.by_ref() {
                    match source_variant.as_str().cmp(raw_variant) {
                        Ordering::Equal => {
                            // The source_variant is equal, move to next raw_variant
                            continue 'outer;
                        }
                        Ordering::Less => {
                            // The source_variant is smaller, take the next source_variant
                        }
                        Ordering::Greater => {
                            // The source_variant is greater,
                            // raw_variants is not a subset of source_variants
                            return false;
                        }
                    }
                }
                // There are raw_variants left after we exhausted source_variants
                return false;
            }
            true
        }
}

fn uts35_replacement<'a, I>(
    source: &mut LanguageIdentifier,
    ruletype_has_language: bool,
    ruletype_has_script: bool,
    ruletype_has_region: bool,
    ruletype_variants: Option<I>,
    replacement: &LanguageIdentifier,
) where
    I: Iterator<Item = &'a str>,
{
    if ruletype_has_language || (source.language.is_unknown() && !replacement.language.is_unknown())
    {
        source.language = replacement.language;
    }
    if ruletype_has_script || (source.script.is_none() && replacement.script.is_some()) {
        source.script = replacement.script;
    }
    if ruletype_has_region || (source.region.is_none() && replacement.region.is_some()) {
        source.region = replacement.region;
    }
    if let Some(skips) = ruletype_variants {
        // The rule matches if the ruletype variants are a subset of the source variants.
        // This means ja-Latn-fonipa-hepburn-heploc matches against the rule for
        // hepburn-heploc and is canonicalized to ja-Latn-alalc97-fonipa

        // We're merging three sorted deduped iterators into a new sequence:
        // sources - skips + replacements

        let mut sources = source.variants.iter().peekable();
        let mut replacements = replacement.variants.iter().peekable();
        let mut skips = skips.peekable();

        let mut variants: Vec<Variant> = Vec::new();

        loop {
            match (sources.peek(), skips.peek(), replacements.peek()) {
                (Some(&source), Some(skip), _)
                    if source.as_str().cmp(skip) == Ordering::Greater =>
                {
                    skips.next();
                }
                (Some(&source), Some(skip), _) if source.as_str().cmp(skip) == Ordering::Equal => {
                    skips.next();
                    sources.next();
                }
                (Some(&source), _, Some(&replacement))
                    if replacement.cmp(source) == Ordering::Less =>
                {
                    variants.push(*replacement);
                    replacements.next();
                }
                (Some(&source), _, Some(&replacement))
                    if replacement.cmp(source) == Ordering::Equal =>
                {
                    variants.push(*source);
                    sources.next();
                    replacements.next();
                }
                (Some(&source), _, _) => {
                    variants.push(*source);
                    sources.next();
                }
                (None, _, Some(&replacement)) => {
                    variants.push(*replacement);
                    replacements.next();
                }
                (None, _, None) => {
                    break;
                }
            }
        }
        source.variants = Variants::from_vec_unchecked(variants);
    }
}

#[inline]
fn uts35_check_language_rules(
    langid: &mut LanguageIdentifier,
    alias_data: &DataPayload<LocaleAliasesV1>,
) -> TransformResult {
    if !langid.language.is_unknown() {
        let lang: TinyAsciiStr<3> = langid.language.into();
        let replacement = if lang.len() == 2 {
            alias_data
                .get()
                .language_len2
                .get(&lang.resize().to_unvalidated())
        } else {
            alias_data.get().language_len3.get(&lang.to_unvalidated())
        };

        if let Some(replacement) = replacement {
            if let Ok(new_langid) = replacement.parse() {
                uts35_replacement::<core::iter::Empty<&str>>(
                    langid,
                    true,
                    false,
                    false,
                    None,
                    &new_langid,
                );
                return TransformResult::Modified;
            }
        }
    }

    TransformResult::Unmodified
}

impl LocaleCanonicalizer<LocaleExpander> {
    /// A constructor which creates a [`LocaleCanonicalizer`] from compiled data,
    /// using a [`LocaleExpander`] for common locales.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    pub const fn new_common() -> Self {
        Self::new_with_expander(LocaleExpander::new_common())
    }

    icu_provider::gen_buffer_data_constructors!(() -> error: DataError,
        functions: [
            new_common: skip,
            try_new_common_with_buffer_provider,
            try_new_common_unstable,
            Self,
        ]
    );

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new_common)]
    pub fn try_new_common_unstable<P>(provider: &P) -> Result<Self, DataError>
    where
        P: DataProvider<LocaleAliasesV1>
            + DataProvider<LocaleLikelySubtagsLanguageV1>
            + DataProvider<LocaleLikelySubtagsScriptRegionV1>
            + ?Sized,
    {
        let expander = LocaleExpander::try_new_common_unstable(provider)?;
        Self::try_new_with_expander_unstable(provider, expander)
    }

    /// A constructor which creates a [`LocaleCanonicalizer`] from compiled data,
    /// using a [`LocaleExpander`] for all locales.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    pub const fn new_extended() -> Self {
        Self::new_with_expander(LocaleExpander::new_extended())
    }

    icu_provider::gen_buffer_data_constructors!(() -> error: DataError,
        functions: [
            new_extended: skip,
            try_new_extended_with_buffer_provider,
            try_new_extended_unstable,
            Self,
        ]
    );

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new_extended)]
    pub fn try_new_extended_unstable<P>(provider: &P) -> Result<Self, DataError>
    where
        P: DataProvider<LocaleAliasesV1>
            + DataProvider<LocaleLikelySubtagsLanguageV1>
            + DataProvider<LocaleLikelySubtagsScriptRegionV1>
            + DataProvider<LocaleLikelySubtagsExtendedV1>
            + ?Sized,
    {
        let expander = LocaleExpander::try_new_extended_unstable(provider)?;
        Self::try_new_with_expander_unstable(provider, expander)
    }
}

impl<Expander: AsRef<LocaleExpander>> LocaleCanonicalizer<Expander> {
    /// Creates a [`LocaleCanonicalizer`] with a custom [`LocaleExpander`] and compiled data.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    pub const fn new_with_expander(expander: Expander) -> Self {
        Self {
            aliases: DataPayload::from_static_ref(
                crate::provider::Baked::SINGLETON_LOCALE_ALIASES_V1,
            ),
            expander,
        }
    }

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new_with_expander)]
    pub fn try_new_with_expander_unstable<P>(
        provider: &P,
        expander: Expander,
    ) -> Result<Self, DataError>
    where
        P: DataProvider<LocaleAliasesV1> + ?Sized,
    {
        let aliases: DataPayload<LocaleAliasesV1> = provider.load(Default::default())?.payload;

        Ok(Self { aliases, expander })
    }

    icu_provider::gen_buffer_data_constructors!((options: Expander) -> error: DataError,
        functions: [
            new_with_expander: skip,
            try_new_with_expander_with_buffer_provider,
            try_new_with_expander_unstable,
            Self,
        ]
    );

    /// The canonicalize method potentially updates a passed in locale in place
    /// depending up the results of running the canonicalization algorithm
    /// from <http://unicode.org/reports/tr35/#LocaleId_Canonicalization>.
    ///
    /// Some BCP47 canonicalization data is not part of the CLDR json package. Because
    /// of this, some canonicalizations are not performed, e.g. the canonicalization of
    /// `und-u-ca-islamicc` to `und-u-ca-islamic-civil`. This will be fixed in a future
    /// release once the missing data has been added to the CLDR json data. See:
    /// <https://github.com/unicode-org/icu4x/issues/746>
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::{Locale, LocaleCanonicalizer, TransformResult};
    ///
    /// let lc = LocaleCanonicalizer::new_extended();
    ///
    /// let mut locale: Locale = "ja-Latn-fonipa-hepburn-heploc".parse().unwrap();
    /// assert_eq!(lc.canonicalize(&mut locale), TransformResult::Modified);
    /// assert_eq!(locale, "ja-Latn-alalc97-fonipa".parse().unwrap());
    /// ```
    pub fn canonicalize(&self, locale: &mut Locale) -> TransformResult {
        let mut result = TransformResult::Unmodified;

        // This loops until we get a 'fixed point', where applying the rules do not
        // result in any more changes.
        loop {
            // These are linear searches due to the ordering imposed by the canonicalization
            // rules, where rules with more variants should be considered first. With the
            // current data in CLDR, we will only do this for locales which have variants,
            // or new rules which we haven't special-cased yet (of which there are fewer
            // than 20).
            let modified = if locale.id.variants.is_empty() {
                self.canonicalize_absolute_language_fallbacks(&mut locale.id)
            } else {
                self.canonicalize_language_variant_fallbacks(&mut locale.id)
            };
            if modified {
                result = TransformResult::Modified;
                continue;
            }

            if !locale.id.language.is_unknown() {
                // If the region is specified, check sgn-region rules first
                if let Some(region) = locale.id.region {
                    if locale.id.language == language!("sgn") {
                        if let Some(&sgn_lang) = self
                            .aliases
                            .get()
                            .sgn_region
                            .get(&region.to_tinystr().to_unvalidated())
                        {
                            uts35_replacement::<core::iter::Empty<&str>>(
                                &mut locale.id,
                                true,
                                false,
                                true,
                                None,
                                &sgn_lang.into(),
                            );
                            result = TransformResult::Modified;
                            continue;
                        }
                    }
                }

                if uts35_check_language_rules(&mut locale.id, &self.aliases)
                    == TransformResult::Modified
                {
                    result = TransformResult::Modified;
                    continue;
                }
            }

            if let Some(script) = locale.id.script {
                if let Some(&replacement) = self
                    .aliases
                    .get()
                    .script
                    .get(&script.to_tinystr().to_unvalidated())
                {
                    locale.id.script = Some(replacement);
                    result = TransformResult::Modified;
                    continue;
                }
            }

            if let Some(region) = locale.id.region {
                let replacement = if region.is_alphabetic() {
                    self.aliases
                        .get()
                        .region_alpha
                        .get(&region.to_tinystr().resize().to_unvalidated())
                } else {
                    self.aliases
                        .get()
                        .region_num
                        .get(&region.to_tinystr().to_unvalidated())
                };
                if let Some(&replacement) = replacement {
                    locale.id.region = Some(replacement);
                    result = TransformResult::Modified;
                    continue;
                }

                if let Some(regions) = self
                    .aliases
                    .get()
                    .complex_region
                    .get(&region.to_tinystr().to_unvalidated())
                {
                    // Skip if regions are empty
                    if let Some(default_region) = regions.get(0) {
                        let mut maximized = LanguageIdentifier {
                            language: locale.id.language,
                            script: locale.id.script,
                            region: None,
                            variants: Variants::default(),
                        };

                        locale.id.region = Some(
                            match (
                                self.expander.as_ref().maximize(&mut maximized),
                                maximized.region,
                            ) {
                                (TransformResult::Modified, Some(candidate))
                                    if regions.iter().any(|x| x == candidate) =>
                                {
                                    candidate
                                }
                                _ => default_region,
                            },
                        );
                        result = TransformResult::Modified;
                        continue;
                    }
                }
            }

            if !locale.id.variants.is_empty() {
                let mut modified = Vec::with_capacity(0);
                for (idx, &variant) in locale.id.variants.iter().enumerate() {
                    if let Some(&updated) = self
                        .aliases
                        .get()
                        .variant
                        .get(&variant.to_tinystr().to_unvalidated())
                    {
                        if modified.is_empty() {
                            modified = locale.id.variants.to_vec();
                        }
                        #[expect(clippy::indexing_slicing)]
                        let _ = core::mem::replace(&mut modified[idx], updated);
                    }
                }

                if !modified.is_empty() {
                    modified.sort();
                    modified.dedup();
                    locale.id.variants = Variants::from_vec_unchecked(modified);
                    result = TransformResult::Modified;
                    continue;
                }
            }

            // Nothing matched in this iteration, we're done.
            break;
        }

        if !locale.extensions.transform.is_empty() || !locale.extensions.unicode.is_empty() {
            self.canonicalize_extensions(&mut locale.extensions, &mut result);
        }
        result
    }

    fn canonicalize_extensions(&self, extensions: &mut Extensions, result: &mut TransformResult) {
        // Handle Locale extensions in their own loops, because these rules do not interact
        // with each other.
        if let Some(ref mut lang) = extensions.transform.lang {
            while uts35_check_language_rules(lang, &self.aliases) == TransformResult::Modified {
                *result = TransformResult::Modified;
            }
        }

        if !extensions.unicode.keywords.is_empty() {
            for key in [key!("rg"), key!("sd")] {
                if let Some(value) = extensions.unicode.keywords.get_mut(&key) {
                    if let Some(only_value) = value.as_single_subtag() {
                        if let Some(modified_value) = self
                            .aliases
                            .get()
                            .subdivision
                            .get(&only_value.to_tinystr().resize().to_unvalidated())
                        {
                            if let Ok(modified_value) = modified_value.parse() {
                                *value = modified_value;
                                *result = TransformResult::Modified;
                            }
                        }
                    }
                }
            }
        }
    }

    fn canonicalize_language_variant_fallbacks(&self, lid: &mut LanguageIdentifier) -> bool {
        // These language/variant comibnations have around 20 rules
        for LanguageStrStrPair(lang, raw_variants, raw_to) in self
            .aliases
            .get()
            .language_variants
            .iter()
            .map(zerofrom::ZeroFrom::zero_from)
        {
            let raw_variants = raw_variants.split('-');
            // if is_iter_sorted(raw_variants.clone()) { // can we sort at construction?
            if uts35_rule_matches(lid, lang, None, None, raw_variants.clone()) {
                if let Ok(to) = raw_to.parse() {
                    uts35_replacement(
                        lid,
                        !lang.is_unknown(),
                        false,
                        false,
                        Some(raw_variants),
                        &to,
                    );
                    return true;
                }
            }
        }
        false
    }

    fn canonicalize_absolute_language_fallbacks(&self, lid: &mut LanguageIdentifier) -> bool {
        for StrStrPair(raw_from, raw_to) in self
            .aliases
            .get()
            .language
            .iter()
            .map(zerofrom::ZeroFrom::zero_from)
        {
            if let Ok(from) = raw_from.parse::<LanguageIdentifier>() {
                if uts35_rule_matches(
                    lid,
                    from.language,
                    from.script,
                    from.region,
                    from.variants.iter().map(Variant::as_str),
                ) {
                    if let Ok(to) = raw_to.parse() {
                        uts35_replacement(
                            lid,
                            !from.language.is_unknown(),
                            from.script.is_some(),
                            from.region.is_some(),
                            Some(from.variants.iter().map(Variant::as_str)),
                            &to,
                        );
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_uts35_rule_matches() {
        for (source, rule, result) in [
            ("ja", "und", true),
            ("und-heploc-hepburn", "und-hepburn", true),
            ("ja-heploc-hepburn", "und-hepburn", true),
            ("ja-hepburn", "und-hepburn-heploc", false),
        ] {
            let source = source.parse().unwrap();
            let rule = rule.parse::<LanguageIdentifier>().unwrap();
            assert_eq!(
                uts35_rule_matches(
                    &source,
                    rule.language,
                    rule.script,
                    rule.region,
                    rule.variants.iter().map(Variant::as_str),
                ),
                result,
                "{source}"
            );
        }
    }

    #[test]
    fn test_uts35_replacement() {
        for (locale, rule_0, rule_1, result) in [
            (
                "ja-Latn-fonipa-hepburn-heploc",
                "und-hepburn-heploc",
                "und-alalc97",
                "ja-Latn-alalc97-fonipa",
            ),
            ("sgn-DD", "und-DD", "und-DE", "sgn-DE"),
            ("sgn-DE", "sgn-DE", "gsg", "gsg"),
        ] {
            let mut locale: Locale = locale.parse().unwrap();
            let rule_0 = rule_0.parse::<LanguageIdentifier>().unwrap();
            let rule_1 = rule_1.parse().unwrap();
            let result = result.parse::<Locale>().unwrap();
            uts35_replacement(
                &mut locale.id,
                !rule_0.language.is_unknown(),
                rule_0.script.is_some(),
                rule_0.region.is_some(),
                Some(rule_0.variants.iter().map(Variant::as_str)),
                &rule_1,
            );
            assert_eq!(result, locale);
        }
    }
}
