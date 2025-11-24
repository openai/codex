// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::LocaleFallbackPriority;
use icu_locale_core::subtags::{Language, Region, Script};

use super::*;

impl LocaleFallbackerWithConfig<'_> {
    pub(crate) fn normalize(&self, locale: &mut DataLocale, default_script: &mut Option<Script>) {
        // 0. If there is an invalid "sd" subtag, drop it
        if let Some(subdivision) = locale.subdivision.take() {
            if let Some(region) = locale.region {
                if subdivision
                    .as_str()
                    .starts_with(region.to_tinystr().to_ascii_lowercase().as_str())
                {
                    locale.subdivision = Some(subdivision);
                }
            }
        }
        let language = locale.language;
        // 1. Populate the region (required for region fallback only)
        if self.config.priority == LocaleFallbackPriority::Region && locale.region.is_none() {
            // 1a. First look for region based on language+script
            if let Some(script) = locale.script {
                locale.region = self
                    .likely_subtags
                    .language_script
                    .get(&(
                        language.to_tinystr().to_unvalidated(),
                        script.to_tinystr().to_unvalidated(),
                    ))
                    .copied();
            }
            // 1b. If that fails, try language only
            if locale.region.is_none() {
                locale.region = self
                    .likely_subtags
                    .language
                    .get_copied(&language.to_tinystr().to_unvalidated())
                    .map(|(_s, r)| r);
            }
        }
        // 2. Remove the script if it is implied by the other subtags
        if locale.script.is_some() || self.config.priority == LocaleFallbackPriority::Script {
            *default_script = locale
                .region
                .and_then(|region| {
                    self.likely_subtags.language_region.get_copied(&(
                        language.to_tinystr().to_unvalidated(),
                        region.to_tinystr().to_unvalidated(),
                    ))
                })
                .or_else(|| {
                    self.likely_subtags
                        .language
                        .get_copied(&language.to_tinystr().to_unvalidated())
                        .map(|(s, _r)| s)
                });
            if locale.script == *default_script {
                locale.script = None;
            }
        }
    }
}

impl LocaleFallbackIteratorInner<'_> {
    pub fn step(&mut self, locale: &mut DataLocale) {
        match self.config.priority {
            LocaleFallbackPriority::Language => self.step_language(locale),
            LocaleFallbackPriority::Script => self.step_script(locale),
            LocaleFallbackPriority::Region => self.step_region(locale),
            // This case should not normally happen, but `LocaleFallbackPriority` is non_exhaustive.
            // Make it go directly to `und`.
            _ => {
                debug_assert!(
                    false,
                    "Unknown LocaleFallbackPriority: {:?}",
                    self.config.priority
                );
                *locale = Default::default()
            }
        }
    }

    fn step_language(&mut self, locale: &mut DataLocale) {
        // 2. Remove the subdivision keyword
        if let Some(value) = locale.subdivision.take() {
            self.backup_subdivision = Some(value);
            return;
        }
        // 4. Remove variants
        if let Some(single_variant) = locale.variant.take() {
            self.backup_variant = Some(single_variant);
            return;
        }
        // 5. Check for parent override
        if let Some((language, script, region)) = self.get_explicit_parent(locale) {
            locale.language = language;
            locale.script = script;
            locale.region = region;
            locale.variant = self.backup_variant.take();
            return;
        }
        // 7. Remove region
        if let Some(region) = locale.region {
            // 6. Add the script subtag if necessary
            if locale.script.is_none() {
                let language = locale.language;
                if let Some(script) = self.likely_subtags.language_region.get_copied(&(
                    language.to_tinystr().to_unvalidated(),
                    region.to_tinystr().to_unvalidated(),
                )) {
                    locale.script = Some(script);
                }
            }
            locale.region = None;
            locale.variant = self.backup_variant.take();
            return;
        }
        // 8. Remove language+script
        debug_assert!(!locale.language.is_unknown() || locale.script.is_some()); // don't call .step() on und
        locale.script = None;
        locale.language = Language::UNKNOWN;
    }

    fn step_region(&mut self, locale: &mut DataLocale) {
        // TODO(#4413): -u-rg is not yet supported
        // 2. Remove the subdivision keyword
        if let Some(value) = locale.subdivision.take() {
            self.backup_subdivision = Some(value);
            return;
        }
        // 4. Remove variants
        if let Some(variant) = locale.variant.take() {
            self.backup_variant = Some(variant);
            return;
        }
        // 5. Remove language+script
        if !locale.language.is_unknown() || locale.script.is_some() {
            locale.script = None;
            locale.language = Language::UNKNOWN;
            // Don't produce und-variant
            if locale.region.is_some() {
                locale.variant = self.backup_variant.take();
                locale.subdivision = self.backup_subdivision.take();
            }
            return;
        }
        // 6. Remove region
        debug_assert!(locale.region.is_some()); // don't call .step() on und
        locale.region = None;
    }

    fn step_script(&mut self, locale: &mut DataLocale) {
        // Remove the subdivision keyword
        if let Some(value) = locale.subdivision.take() {
            self.backup_subdivision = Some(value);
            return;
        }
        // Remove variants
        if let Some(variant) = locale.variant.take() {
            self.backup_variant = Some(variant);
            return;
        }
        // Check for parent override
        if let Some((language, script, region)) = self.get_explicit_parent(locale) {
            locale.language = language;
            locale.script = script;
            locale.region = region;
            locale.variant = self.backup_variant.take();
            return;
        }
        // Remove the region
        if let Some(region) = locale.region {
            self.backup_region = Some(region);
            let language_implied_script = self
                .likely_subtags
                .language
                .get_copied(&locale.language.to_tinystr().to_unvalidated())
                .map(|(s, _r)| s);
            if language_implied_script != self.max_script {
                locale.script = self.max_script;
            }
            locale.region = None;
            locale.variant = self.backup_variant.take();
            return;
        }

        // Remove the script if we have a language
        if !locale.language.is_unknown() {
            let language_implied_script = self
                .likely_subtags
                .language
                .get_copied(&locale.language.to_tinystr().to_unvalidated())
                .map(|(s, _r)| s);
            if locale.script.is_some() && language_implied_script == locale.script {
                locale.script = None;
                if let Some(region) = self.backup_region.take() {
                    locale.region = Some(region);
                    locale.subdivision = self.backup_subdivision.take();
                    locale.variant = self.backup_variant.take();
                }
                // needed if more fallback is added at the end
                return;
            } else {
                // 3. Remove the language and apply the maximized script
                locale.language = Language::UNKNOWN;
                locale.script = self.max_script;
                // Don't produce und-variant
                if locale.script.is_some() {
                    locale.variant = self.backup_variant.take();
                }
                // needed if more fallback is added at the end
                return;
            }
        }

        // note: UTS #35 wants us to apply "other associated scripts" now. ICU4C/J does not do this,
        // so we don't either. They would be found here if they are ever needed:
        // https://github.com/unicode-cldr/cldr-core/blob/master/supplemental/languageData.json

        // 6. Remove script
        if locale.script.is_some() {
            locale.script = None;
        }
    }

    fn get_explicit_parent(
        &self,
        locale: &DataLocale,
    ) -> Option<(Language, Option<Script>, Option<Region>)> {
        self.parents
            .parents
            .get_copied_by(|uvstr| locale.strict_cmp(uvstr).reverse())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use writeable::Writeable;

    struct TestCase {
        input: &'static str,
        requires_data: bool,
        // Note: The first entry in the chain is the normalized locale
        expected_language_chain: &'static [&'static str],
        expected_script_chain: &'static [&'static str],
        expected_region_chain: &'static [&'static str],
    }

    // TODO: Consider loading these from a JSON file
    const TEST_CASES: &[TestCase] = &[
        TestCase {
            input: "en-fonipa",
            requires_data: false,
            expected_language_chain: &["en-fonipa", "en"],
            expected_script_chain: &["en-fonipa", "en"],
            expected_region_chain: &["en-fonipa", "en"],
        },
        TestCase {
            input: "en-US-u-sd-usca",
            requires_data: false,
            expected_language_chain: &["en-US-u-sd-usca", "en-US", "en"],
            expected_script_chain: &["en-US-u-sd-usca", "en-US", "en"],
            expected_region_chain: &["en-US-u-sd-usca", "en-US", "und-US-u-sd-usca", "und-US"],
        },
        TestCase {
            input: "en-US-fonipa-u-sd-usca",
            requires_data: false,
            expected_language_chain: &[
                "en-US-fonipa-u-sd-usca",
                "en-US-fonipa",
                "en-US",
                "en-fonipa",
                "en",
            ],
            expected_script_chain: &[
                "en-US-fonipa-u-sd-usca",
                "en-US-fonipa",
                "en-US",
                "en-fonipa",
                "en",
            ],
            expected_region_chain: &[
                "en-US-fonipa-u-sd-usca",
                "en-US-fonipa",
                "en-US",
                "und-US-fonipa-u-sd-usca",
                "und-US-fonipa",
                "und-US",
            ],
        },
        TestCase {
            input: "en-fonipa",
            requires_data: true,
            expected_language_chain: &["en-fonipa", "en"],
            expected_script_chain: &["en-fonipa", "en", "und-Latn-fonipa", "und-Latn"],
            expected_region_chain: &["en-US-fonipa", "en-US", "und-US-fonipa", "und-US"],
        },
        TestCase {
            input: "en-Latn-fonipa",
            requires_data: true,
            expected_language_chain: &["en-fonipa", "en"],
            expected_script_chain: &["en-fonipa", "en", "und-Latn-fonipa", "und-Latn"],
            expected_region_chain: &["en-US-fonipa", "en-US", "und-US-fonipa", "und-US"],
        },
        TestCase {
            input: "en-Latn-US-u-sd-usca",
            requires_data: true,
            expected_language_chain: &["en-US-u-sd-usca", "en-US", "en"],
            expected_script_chain: &["en-US-u-sd-usca", "en-US", "en", "und-Latn"],
            expected_region_chain: &["en-US-u-sd-usca", "en-US", "und-US-u-sd-usca", "und-US"],
        },
        TestCase {
            input: "sr-ME",
            requires_data: true,
            expected_language_chain: &["sr-ME", "sr-Latn"],
            expected_script_chain: &["sr-ME", "sr-Latn", "und-Latn"],
            expected_region_chain: &["sr-ME", "und-ME"],
        },
        TestCase {
            input: "sr-Latn-ME",
            requires_data: true,
            expected_language_chain: &["sr-ME", "sr-Latn"],
            expected_script_chain: &["sr-ME", "sr-Latn", "und-Latn"],
            expected_region_chain: &["sr-ME", "und-ME"],
        },
        TestCase {
            input: "sr-ME-fonipa",
            requires_data: true,
            expected_language_chain: &["sr-ME-fonipa", "sr-ME", "sr-Latn-fonipa", "sr-Latn"],
            expected_script_chain: &[
                "sr-ME-fonipa",
                "sr-ME",
                "sr-Latn-fonipa",
                "sr-Latn",
                "und-Latn-fonipa",
                "und-Latn",
            ],
            expected_region_chain: &["sr-ME-fonipa", "sr-ME", "und-ME-fonipa", "und-ME"],
        },
        TestCase {
            input: "sr-RS",
            requires_data: true,
            expected_language_chain: &["sr-RS", "sr"],
            expected_script_chain: &["sr-RS", "sr", "und-Cyrl"],
            expected_region_chain: &["sr-RS", "und-RS"],
        },
        TestCase {
            input: "sr-Cyrl-RS",
            requires_data: true,
            expected_language_chain: &["sr-RS", "sr"],
            expected_script_chain: &["sr-RS", "sr", "und-Cyrl"],
            expected_region_chain: &["sr-RS", "und-RS"],
        },
        TestCase {
            input: "sr-Latn-RS",
            requires_data: true,
            expected_language_chain: &["sr-Latn-RS", "sr-Latn"],
            expected_script_chain: &["sr-Latn-RS", "sr-Latn", "und-Latn"],
            expected_region_chain: &["sr-Latn-RS", "und-RS"],
        },
        TestCase {
            input: "de-Latn-LI",
            requires_data: true,
            expected_language_chain: &["de-LI", "de"],
            expected_script_chain: &["de-LI", "de", "und-Latn"],
            expected_region_chain: &["de-LI", "und-LI"],
        },
        TestCase {
            input: "ca-ES-valencia",
            requires_data: true,
            expected_language_chain: &["ca-ES-valencia", "ca-ES", "ca-valencia", "ca"],
            expected_script_chain: &[
                "ca-ES-valencia",
                "ca-ES",
                "ca-valencia",
                "ca",
                "und-Latn-valencia",
                "und-Latn",
            ],
            expected_region_chain: &["ca-ES-valencia", "ca-ES", "und-ES-valencia", "und-ES"],
        },
        TestCase {
            input: "es-AR",
            requires_data: true,
            expected_language_chain: &["es-AR", "es-419", "es"],
            expected_script_chain: &["es-AR", "es-419", "es", "und-Latn"],
            expected_region_chain: &["es-AR", "und-AR"],
        },
        TestCase {
            input: "hi-IN",
            requires_data: true,
            expected_language_chain: &["hi-IN", "hi"],
            expected_script_chain: &["hi-IN", "hi", "und-Deva"],
            expected_region_chain: &["hi-IN", "und-IN"],
        },
        TestCase {
            input: "hi-Latn-IN",
            requires_data: true,
            expected_language_chain: &["hi-Latn-IN", "hi-Latn", "en-IN", "en-001", "en"],
            expected_script_chain: &["hi-Latn-IN", "hi-Latn", "en-IN", "en-001", "en", "und-Latn"],
            expected_region_chain: &["hi-Latn-IN", "und-IN"],
        },
        TestCase {
            input: "zh-CN",
            requires_data: true,
            // Note: "zh-Hans" is not reachable because it is the default script for "zh".
            // The fallback algorithm does not visit the language-script bundle when the
            // script is the default for the language
            expected_language_chain: &["zh-CN", "zh"],
            expected_script_chain: &["zh-CN", "zh", "und-Hans", "und-Hani"],
            expected_region_chain: &["zh-CN", "und-CN"],
        },
        TestCase {
            input: "zh-TW",
            requires_data: true,
            expected_language_chain: &["zh-TW", "zh-Hant"],
            expected_script_chain: &["zh-TW", "zh-Hant", "und-Hant", "und-Hani"],
            expected_region_chain: &["zh-TW", "und-TW"],
        },
        TestCase {
            input: "yue-HK",
            requires_data: true,
            expected_language_chain: &["yue-HK", "yue"],
            expected_script_chain: &["yue-HK", "yue", "und-Hant", "und-Hani"],
            expected_region_chain: &["yue-HK", "und-HK"],
        },
        TestCase {
            input: "yue-HK",
            requires_data: true,
            expected_language_chain: &["yue-HK", "yue"],
            expected_script_chain: &["yue-HK", "yue", "und-Hant", "und-Hani"],
            expected_region_chain: &["yue-HK", "und-HK"],
        },
        TestCase {
            input: "yue-CN",
            requires_data: true,
            expected_language_chain: &["yue-CN", "yue-Hans"],
            expected_script_chain: &["yue-CN", "yue-Hans", "und-Hans", "und-Hani"],
            expected_region_chain: &["yue-CN", "und-CN"],
        },
        TestCase {
            input: "az-Arab-IR",
            requires_data: true,
            expected_language_chain: &["az-IR", "az-Arab"],
            expected_script_chain: &["az-IR", "az-Arab", "und-Arab"],
            expected_region_chain: &["az-IR", "und-IR"],
        },
        TestCase {
            input: "az-IR",
            requires_data: true,
            expected_language_chain: &["az-IR", "az-Arab"],
            expected_script_chain: &["az-IR", "az-Arab", "und-Arab"],
            expected_region_chain: &["az-IR", "und-IR"],
        },
        TestCase {
            input: "az-Arab",
            requires_data: true,
            expected_language_chain: &["az-Arab"],
            expected_script_chain: &["az-Arab", "und-Arab"],
            expected_region_chain: &["az-IR", "und-IR"],
        },
    ];

    #[test]
    fn test_fallback() {
        let fallbacker_no_data = LocaleFallbacker::new_without_data();
        let fallbacker_no_data = fallbacker_no_data.as_borrowed();
        let fallbacker_with_data = LocaleFallbacker::new();
        for cas in TEST_CASES {
            for (priority, expected_chain) in [
                (
                    LocaleFallbackPriority::Language,
                    cas.expected_language_chain,
                ),
                (LocaleFallbackPriority::Script, cas.expected_script_chain),
                (LocaleFallbackPriority::Region, cas.expected_region_chain),
            ] {
                let mut config = LocaleFallbackConfig::default();
                config.priority = priority;
                let fallbacker = if cas.requires_data {
                    fallbacker_with_data
                } else {
                    fallbacker_no_data
                };
                let mut it = fallbacker
                    .for_config(config)
                    .fallback_for(cas.input.parse().unwrap());
                let mut actual_chain = Vec::new();
                for i in 0..20 {
                    if i == 19 {
                        eprintln!("20 iterations reached!");
                    }
                    if it.get().is_unknown() {
                        break;
                    }
                    actual_chain.push(it.get().write_to_string().into_owned());
                    it.step();
                }
                assert_eq!(
                    expected_chain, &actual_chain,
                    "{:?} ({:?})",
                    cas.input, priority
                );
            }
        }
    }
}
