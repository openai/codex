// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::provider::*;
use crate::LocaleExpander;
use icu_locale_core::subtags::Script;
use icu_locale_core::LanguageIdentifier;
use icu_provider::prelude::*;

/// Represents the direction of a script.
///
/// [`LocaleDirectionality`] can be used to get this information.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[non_exhaustive]
pub enum Direction {
    /// The script is left-to-right.
    LeftToRight,
    /// The script is right-to-left.
    RightToLeft,
}

/// Provides methods to determine the direction of a locale.
///
/// The `Expander` generic parameter wraps a [`LocaleExpander`].
///
/// # Examples
///
/// ```
/// use icu::locale::{langid, Direction, LocaleDirectionality};
///
/// let ld = LocaleDirectionality::new_common();
///
/// assert_eq!(ld.get(&langid!("en")), Some(Direction::LeftToRight));
/// ```
#[derive(Debug)]
pub struct LocaleDirectionality<Expander = LocaleExpander> {
    script_direction: DataPayload<LocaleScriptDirectionV1>,
    expander: Expander,
}

impl LocaleDirectionality<LocaleExpander> {
    /// Creates a [`LocaleDirectionality`] from compiled data, using [`LocaleExpander`]
    /// data for common locales.
    ///
    /// This includes limited likely subtags data, see [`LocaleExpander::new_common()`].
    #[cfg(feature = "compiled_data")]
    pub const fn new_common() -> Self {
        Self::new_with_expander(LocaleExpander::new_common())
    }

    // Note: This is a custom impl because the bounds on `try_new_unstable` don't suffice
    #[doc = icu_provider::gen_buffer_unstable_docs!(BUFFER, Self::new_common)]
    #[cfg(feature = "serde")]
    pub fn try_new_common_with_buffer_provider(
        provider: &(impl BufferProvider + ?Sized),
    ) -> Result<Self, DataError> {
        let expander = LocaleExpander::try_new_common_with_buffer_provider(provider)?;
        Self::try_new_with_expander_unstable(&provider.as_deserializing(), expander)
    }
    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new_common)]
    pub fn try_new_common_unstable<P>(provider: &P) -> Result<LocaleDirectionality, DataError>
    where
        P: DataProvider<LocaleScriptDirectionV1>
            + DataProvider<LocaleLikelySubtagsLanguageV1>
            + DataProvider<LocaleLikelySubtagsScriptRegionV1>
            + ?Sized,
    {
        let expander = LocaleExpander::try_new_common_unstable(provider)?;
        Self::try_new_with_expander_unstable(provider, expander)
    }

    /// Creates a [`LocaleDirectionality`] from compiled data, using [`LocaleExpander`]
    /// data for all locales.
    ///
    /// This includes all likely subtags data, see [`LocaleExpander::new_extended()`].
    #[cfg(feature = "compiled_data")]
    pub const fn new_extended() -> Self {
        Self::new_with_expander(LocaleExpander::new_extended())
    }

    // Note: This is a custom impl because the bounds on `try_new_unstable` don't suffice
    #[doc = icu_provider::gen_buffer_unstable_docs!(BUFFER, Self::new_extended)]
    #[cfg(feature = "serde")]
    pub fn try_new_extended_with_buffer_provider(
        provider: &(impl BufferProvider + ?Sized),
    ) -> Result<Self, DataError> {
        let expander = LocaleExpander::try_new_extended_with_buffer_provider(provider)?;
        Self::try_new_with_expander_unstable(&provider.as_deserializing(), expander)
    }
    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new_extended)]
    pub fn try_new_extended_unstable<P>(provider: &P) -> Result<LocaleDirectionality, DataError>
    where
        P: DataProvider<LocaleScriptDirectionV1>
            + DataProvider<LocaleLikelySubtagsLanguageV1>
            + DataProvider<LocaleLikelySubtagsScriptRegionV1>
            + DataProvider<LocaleLikelySubtagsExtendedV1>
            + ?Sized,
    {
        let expander = LocaleExpander::try_new_extended_unstable(provider)?;
        Self::try_new_with_expander_unstable(provider, expander)
    }
}

impl<Expander: AsRef<LocaleExpander>> LocaleDirectionality<Expander> {
    /// Creates a [`LocaleDirectionality`] with a custom [`LocaleExpander`] and compiled data.
    ///
    /// This allows using [`LocaleExpander::new_extended()`] with data for all locales.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::{
    ///     langid, Direction, LocaleDirectionality, LocaleExpander,
    /// };
    ///
    /// let ld_default = LocaleDirectionality::new_common();
    ///
    /// assert_eq!(ld_default.get(&langid!("jbn")), None);
    ///
    /// let expander = LocaleExpander::new_extended();
    /// let ld_extended = LocaleDirectionality::new_with_expander(expander);
    ///
    /// assert_eq!(
    ///     ld_extended.get(&langid!("jbn")),
    ///     Some(Direction::RightToLeft)
    /// );
    /// ```
    #[cfg(feature = "compiled_data")]
    pub const fn new_with_expander(expander: Expander) -> Self {
        LocaleDirectionality {
            script_direction: DataPayload::from_static_ref(
                crate::provider::Baked::SINGLETON_LOCALE_SCRIPT_DIRECTION_V1,
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
        P: DataProvider<LocaleScriptDirectionV1> + ?Sized,
    {
        let script_direction = provider.load(Default::default())?.payload;

        Ok(LocaleDirectionality {
            script_direction,
            expander,
        })
    }

    /// Returns the script direction of the given locale.
    ///
    /// Note that the direction is a property of the script of a locale, not of the language. As such,
    /// when given a locale without an associated script tag (i.e., `locale!("en")` vs. `locale!("en-Latn")`),
    /// this method first tries to infer the script using the language and region before returning its direction.
    ///
    /// If you already have a script struct and want to get its direction, you should use
    /// `Locale::from(Some(my_script))` and call this method.
    ///
    /// This method will return `None` if either a locale's script cannot be determined, or there is no information
    /// for the script.
    ///
    /// # Examples
    ///
    /// Using an existing locale:
    ///
    /// ```
    /// use icu::locale::{langid, Direction, LocaleDirectionality};
    ///
    /// let ld = LocaleDirectionality::new_common();
    ///
    /// assert_eq!(ld.get(&langid!("en-US")), Some(Direction::LeftToRight));
    ///
    /// assert_eq!(ld.get(&langid!("ar")), Some(Direction::RightToLeft));
    ///
    /// assert_eq!(ld.get(&langid!("en-Arab")), Some(Direction::RightToLeft));
    ///
    /// assert_eq!(ld.get(&langid!("foo")), None);
    /// ```
    ///
    /// Using a script directly:
    ///
    /// ```
    /// use icu::locale::subtags::script;
    /// use icu::locale::{Direction, LanguageIdentifier, LocaleDirectionality};
    ///
    /// let ld = LocaleDirectionality::new_common();
    ///
    /// assert_eq!(
    ///     ld.get(&LanguageIdentifier::from(Some(script!("Latn")))),
    ///     Some(Direction::LeftToRight)
    /// );
    /// ```
    pub fn get(&self, langid: &LanguageIdentifier) -> Option<Direction> {
        let script = self.expander.as_ref().get_likely_script(langid)?;

        if self.script_in_ltr(script) {
            Some(Direction::LeftToRight)
        } else if self.script_in_rtl(script) {
            Some(Direction::RightToLeft)
        } else {
            None
        }
    }

    /// Returns whether the given locale is right-to-left.
    ///
    /// Note that if this method returns `false`, the locale is either left-to-right or
    /// the [`LocaleDirectionality`] does not include data for the locale.
    /// You should use [`LocaleDirectionality::get`] if you need to differentiate between these cases.
    ///
    /// See [`LocaleDirectionality::get`] for more information.
    pub fn is_right_to_left(&self, langid: &LanguageIdentifier) -> bool {
        self.expander
            .as_ref()
            .get_likely_script(langid)
            .map(|s| self.script_in_rtl(s))
            .unwrap_or(false)
    }

    /// Returns whether the given locale is left-to-right.
    ///
    /// Note that if this method returns `false`, the locale is either right-to-left or
    /// the [`LocaleDirectionality`] does not include data for the locale.
    /// You should use [`LocaleDirectionality::get`] if you need to differentiate between these cases.
    ///
    /// See [`LocaleDirectionality::get`] for more information.
    pub fn is_left_to_right(&self, langid: &LanguageIdentifier) -> bool {
        self.expander
            .as_ref()
            .get_likely_script(langid)
            .map(|s| self.script_in_ltr(s))
            .unwrap_or(false)
    }

    fn script_in_rtl(&self, script: Script) -> bool {
        self.script_direction
            .get()
            .rtl
            .binary_search(&script.to_tinystr().to_unvalidated())
            .is_ok()
    }

    fn script_in_ltr(&self, script: Script) -> bool {
        self.script_direction
            .get()
            .ltr
            .binary_search(&script.to_tinystr().to_unvalidated())
            .is_ok()
    }
}
