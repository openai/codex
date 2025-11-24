// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#[cfg(feature = "alloc")]
use crate::subtags::Variants;
use crate::subtags::{Language, Region, Script, Subtag, Variant};
use crate::DataLocale;

/// The structure storing locale subtags used in preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalePreferences {
    /// Preference of Language
    pub(crate) language: Language,
    /// Preference of Script
    pub(crate) script: Option<Script>,
    /// Preference of Region
    pub(crate) region: Option<Region>,
    /// Preference of Variant
    pub(crate) variant: Option<Variant>,
    /// Preference of Regional Subdivision
    pub(crate) subdivision: Option<Subtag>,
    /// Preference of Unicode Extension Region
    pub(crate) ue_region: Option<Region>,
}

impl LocalePreferences {
    fn to_data_locale_maybe_region_priority(self, region_priority: bool) -> DataLocale {
        DataLocale {
            language: self.language,
            script: self.script,
            region: match (self.region, self.ue_region) {
                (Some(_), Some(r)) if region_priority => Some(r),
                (r, _) => r,
            },
            variant: self.variant,
            subdivision: self.subdivision,
        }
    }

    /// Convert to a DataLocale, with region-based fallback priority
    ///
    /// Most users should use `icu_provider::marker::make_locale()` instead.
    pub fn to_data_locale_region_priority(self) -> DataLocale {
        self.to_data_locale_maybe_region_priority(true)
    }

    /// Convert to a DataLocale, with language-based fallback priority
    ///
    /// Most users should use `icu_provider::marker::make_locale()` instead.
    pub fn to_data_locale_language_priority(self) -> DataLocale {
        self.to_data_locale_maybe_region_priority(false)
    }
}
impl Default for LocalePreferences {
    fn default() -> Self {
        Self::default()
    }
}

impl From<&crate::Locale> for LocalePreferences {
    fn from(loc: &crate::Locale) -> Self {
        let sd = loc
            .extensions
            .unicode
            .keywords
            .get(&crate::extensions::unicode::key!("sd"))
            .and_then(|v| v.as_single_subtag().copied());
        let ue_region = loc
            .extensions
            .unicode
            .keywords
            .get(&crate::extensions::unicode::key!("rg"))
            .and_then(|v| {
                v.as_single_subtag()
                    .and_then(|s| Region::try_from_str(s.as_str()).ok())
            });
        Self {
            language: loc.id.language,
            script: loc.id.script,
            region: loc.id.region,
            variant: loc.id.variants.iter().copied().next(),
            subdivision: sd,
            ue_region,
        }
    }
}

impl From<&crate::LanguageIdentifier> for LocalePreferences {
    fn from(lid: &crate::LanguageIdentifier) -> Self {
        Self {
            language: lid.language,
            script: lid.script,
            region: lid.region,
            variant: lid.variants.iter().copied().next(),
            subdivision: None,
            ue_region: None,
        }
    }
}

/// ✨ *Enabled with the `alloc` Cargo feature.*
#[cfg(feature = "alloc")]
impl From<LocalePreferences> for crate::Locale {
    fn from(prefs: LocalePreferences) -> Self {
        Self {
            id: crate::LanguageIdentifier {
                language: prefs.language,
                script: prefs.script,
                region: prefs.region,
                variants: prefs
                    .variant
                    .map(Variants::from_variant)
                    .unwrap_or_default(),
            },
            extensions: {
                let mut extensions = crate::extensions::Extensions::default();
                if let Some(sd) = prefs.subdivision {
                    extensions.unicode.keywords.set(
                        crate::extensions::unicode::key!("sd"),
                        crate::extensions::unicode::Value::from_subtag(Some(sd)),
                    );
                }
                if let Some(rg) = prefs.ue_region {
                    #[expect(clippy::unwrap_used)] // Region is a valid Subtag
                    extensions.unicode.keywords.set(
                        crate::extensions::unicode::key!("rg"),
                        crate::extensions::unicode::Value::try_from_str(rg.as_str()).unwrap(),
                    );
                }
                extensions
            },
        }
    }
}

impl LocalePreferences {
    /// Constructs a new [`LocalePreferences`] struct with the defaults.
    pub const fn default() -> Self {
        Self {
            language: Language::UNKNOWN,
            script: None,
            region: None,
            variant: None,
            subdivision: None,
            ue_region: None,
        }
    }

    /// Preference of Language
    pub const fn language(&self) -> Language {
        self.language
    }

    /// Preference of Region
    pub const fn region(&self) -> Option<Region> {
        self.region
    }

    /// Extends the preferences with the values from another set of preferences.
    pub fn extend(&mut self, other: LocalePreferences) {
        if !other.language.is_unknown() {
            self.language = other.language;
        }
        if let Some(script) = other.script {
            self.script = Some(script);
        }
        if let Some(region) = other.region {
            self.region = Some(region);
        }
        if let Some(variant) = other.variant {
            self.variant = Some(variant);
        }
        if let Some(sd) = other.subdivision {
            self.subdivision = Some(sd);
        }
        if let Some(ue_region) = other.ue_region {
            self.ue_region = Some(ue_region);
        }
    }
}
