// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Options to define fallback behaviour.
//!
//! These options are consumed by the `LocaleFallbacker` in the `icu_locales` crate
//! (or the `icu::locales` module), but are defined here because they are used by `DataMarkerInfo`.

/// Hint for which subtag to prioritize during fallback.
///
/// For example, `"en-US"` might fall back to either `"en"` or `"und-US"` depending
/// on this enum.
#[derive(Debug, PartialEq, Eq, Copy, Clone, PartialOrd, Ord)]
#[non_exhaustive]
pub enum LocaleFallbackPriority {
    /// Prioritize the language. This is the default behavior.
    ///
    /// For example, `"en-US"` should go to `"en"` and then `"und"`.
    Language,
    /// Prioritize the script.
    ///
    /// For example, `"en-US"` should go to `"en"` and then `"und-Latn"` and then `"und"`.
    Script,
    /// Prioritize the region.
    ///
    /// For example, `"en-US"` should go to `"und-US"` and then `"und"`.
    Region,
}

impl LocaleFallbackPriority {
    /// Const-friendly version of [`Default::default`].
    pub const fn default() -> Self {
        Self::Language
    }
}

impl Default for LocaleFallbackPriority {
    fn default() -> Self {
        Self::default()
    }
}

/// Configuration settings for a particular fallback operation.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[non_exhaustive]
pub struct LocaleFallbackConfig {
    /// Strategy for choosing which subtags to drop during locale fallback.
    ///
    /// # Examples
    ///
    /// Retain the language and script subtags until the final step:
    ///
    /// ```
    /// use icu::locale::fallback::LocaleFallbackConfig;
    /// use icu::locale::fallback::LocaleFallbackPriority;
    /// use icu::locale::locale;
    /// use icu::locale::LocaleFallbacker;
    ///
    /// // Set up the fallback iterator.
    /// let fallbacker = LocaleFallbacker::new();
    /// let mut config = LocaleFallbackConfig::default();
    /// config.priority = LocaleFallbackPriority::Language;
    /// let mut fallback_iterator = fallbacker
    ///     .for_config(config)
    ///     .fallback_for(locale!("ca-ES-valencia").into());
    ///
    /// // Run the algorithm and check the results.
    /// assert_eq!(fallback_iterator.get(), &locale!("ca-ES-valencia").into());
    /// fallback_iterator.step();
    /// assert_eq!(fallback_iterator.get(), &locale!("ca-ES").into());
    /// fallback_iterator.step();
    /// assert_eq!(fallback_iterator.get(), &locale!("ca-valencia").into());
    /// fallback_iterator.step();
    /// assert_eq!(fallback_iterator.get(), &locale!("ca").into());
    /// fallback_iterator.step();
    /// assert_eq!(fallback_iterator.get(), &locale!("und").into());
    /// ```
    ///
    /// Retain the region subtag until the final step:
    ///
    /// ```
    /// use icu::locale::fallback::LocaleFallbackConfig;
    /// use icu::locale::fallback::LocaleFallbackPriority;
    /// use icu::locale::locale;
    /// use icu::locale::LocaleFallbacker;
    ///
    /// // Set up the fallback iterator.
    /// let fallbacker = LocaleFallbacker::new();
    /// let mut config = LocaleFallbackConfig::default();
    /// config.priority = LocaleFallbackPriority::Region;
    /// let mut fallback_iterator = fallbacker
    ///     .for_config(config)
    ///     .fallback_for(locale!("ca-ES-valencia").into());
    ///
    /// // Run the algorithm and check the results.
    /// assert_eq!(fallback_iterator.get(), &locale!("ca-ES-valencia").into());
    /// fallback_iterator.step();
    /// assert_eq!(fallback_iterator.get(), &locale!("ca-ES").into());
    /// fallback_iterator.step();
    /// assert_eq!(fallback_iterator.get(), &locale!("und-ES-valencia").into());
    /// fallback_iterator.step();
    /// assert_eq!(fallback_iterator.get(), &locale!("und-ES").into());
    /// fallback_iterator.step();
    /// assert_eq!(fallback_iterator.get(), &locale!("und").into());
    /// ```
    pub priority: LocaleFallbackPriority,
}

impl LocaleFallbackConfig {
    /// Const version of [`Default::default`].
    pub const fn default() -> Self {
        Self {
            priority: LocaleFallbackPriority::default(),
        }
    }
}

impl Default for LocaleFallbackConfig {
    fn default() -> Self {
        Self::default()
    }
}
