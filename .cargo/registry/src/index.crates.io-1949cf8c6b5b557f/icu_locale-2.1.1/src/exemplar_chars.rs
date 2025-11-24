// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This module provides APIs for getting exemplar characters for a locale.
//!
//! Exemplars are characters used by a language, separated into different sets.
//! The sets are: main, auxiliary, punctuation, numbers, and index.
//!
//! The sets define, according to typical usage in the language,
//! which characters occur in which contexts with which frequency.
//! For more information, see the documentation in the
//! [Exemplars section in Unicode Technical Standard #35](https://unicode.org/reports/tr35/tr35-general.html#Exemplars)
//! of the LDML specification.
//!
//! # Examples
//!
//! ```
//! use icu::locale::exemplar_chars::ExemplarCharacters;
//! use icu::locale::locale;
//!
//! let locale = locale!("en-001").into();
//! let exemplars_main = ExemplarCharacters::try_new_main(&locale)
//!     .expect("locale should be present");
//!
//! assert!(exemplars_main.contains('a'));
//! assert!(exemplars_main.contains('z'));
//! assert!(exemplars_main.contains_str("a"));
//! assert!(!exemplars_main.contains_str("ä"));
//! assert!(!exemplars_main.contains_str("ng"));
//! ```

use crate::provider::*;
use core::ops::Deref;
use icu_collections::codepointinvliststringlist::CodePointInversionListAndStringList;
use icu_provider::{marker::ErasedMarker, prelude::*};

/// A wrapper around `UnicodeSet` data (characters and strings)
#[derive(Debug)]
pub struct ExemplarCharacters {
    data: DataPayload<ErasedMarker<ExemplarCharactersData<'static>>>,
}

impl ExemplarCharacters {
    /// Construct a borrowed version of this type that can be queried.
    ///
    /// This avoids a potential small underlying cost per API call (ex: `contains()`) by consolidating it
    /// up front.
    #[inline]
    pub fn as_borrowed(&self) -> ExemplarCharactersBorrowed<'_> {
        ExemplarCharactersBorrowed {
            data: self.data.get(),
        }
    }
}

/// A borrowed wrapper around code point set data, returned by
/// [`ExemplarCharacters::as_borrowed()`]. More efficient to query.
#[derive(Clone, Copy, Debug)]
pub struct ExemplarCharactersBorrowed<'a> {
    data: &'a ExemplarCharactersData<'a>,
}

impl<'a> Deref for ExemplarCharactersBorrowed<'a> {
    type Target = CodePointInversionListAndStringList<'a>;

    fn deref(&self) -> &Self::Target {
        &self.data.0
    }
}

impl ExemplarCharactersBorrowed<'static> {
    /// Cheaply converts a [`ExemplarCharactersBorrowed<'static>`] into a [`ExemplarCharacters`].
    ///
    /// Note: Due to branching and indirection, using [`ExemplarCharacters`] might inhibit some
    /// compile-time optimizations that are possible with [`ExemplarCharactersBorrowed`].
    pub const fn static_to_owned(self) -> ExemplarCharacters {
        ExemplarCharacters {
            data: DataPayload::from_static_ref(self.data),
        }
    }
}

macro_rules! make_exemplar_chars_unicode_set_property {
    (
        // currently unused
        dyn_data_marker: $d:ident;
        data_marker: $data_marker:ty;
        func:
        pub fn $unstable:ident();
        $(#[$attr:meta])*
        pub fn $compiled:ident();
    ) => {
        impl ExemplarCharactersBorrowed<'static> {
            $(#[$attr])*
            #[cfg(feature = "compiled_data")]
            #[inline]
            pub fn $compiled(
                locale: &DataLocale,
            ) -> Result<Self, DataError> {
                Ok(ExemplarCharactersBorrowed {
                    data: DataProvider::<$data_marker>::load(
                        &crate::provider::Baked,
                        DataRequest {
                            id: DataIdentifierBorrowed::for_locale(locale),
                            ..Default::default()
                        })?
                    .payload
                    .get_static()
                    .ok_or_else(|| DataError::custom("Baked provider didn't return static payload"))?
                })
            }

        }
        impl ExemplarCharacters {
            $(#[$attr])*
            #[cfg(feature = "compiled_data")]
            pub fn $compiled(
                locale: &DataLocale,
            ) -> Result<ExemplarCharactersBorrowed<'static>, DataError> {
                ExemplarCharactersBorrowed::$compiled(locale)
            }

            #[doc = concat!("A version of [`Self::", stringify!($compiled), "()`] that uses custom data provided by a [`DataProvider`].")]
            ///
            /// [📚 Help choosing a constructor](icu_provider::constructors)
            pub fn $unstable(
                provider: &(impl DataProvider<$data_marker> + ?Sized),
                locale: &DataLocale,
            ) -> Result<Self, DataError> {
                Ok(Self {
                    data:
                    provider.load(
                        DataRequest {
                            id: DataIdentifierBorrowed::for_locale(locale),
                            ..Default::default()
                    })?
                    .payload
                    .cast()
                })
            }
        }
    }
}

make_exemplar_chars_unicode_set_property!(
    dyn_data_marker: ExemplarCharactersMain;
    data_marker: LocaleExemplarCharactersMainV1;
    func:
    pub fn try_new_main_unstable();

    /// Get the "main" set of exemplar characters.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::locale;
    /// use icu::locale::exemplar_chars::ExemplarCharacters;
    ///
    /// let exemplars_main = ExemplarCharacters::try_new_main(&locale!("en").into())
    ///     .expect("locale should be present");
    ///
    /// assert!(exemplars_main.contains('a'));
    /// assert!(exemplars_main.contains('z'));
    /// assert!(exemplars_main.contains_str("a"));
    /// assert!(!exemplars_main.contains_str("ä"));
    /// assert!(!exemplars_main.contains_str("ng"));
    /// assert!(!exemplars_main.contains_str("A"));
    /// ```
    pub fn try_new_main();
);

make_exemplar_chars_unicode_set_property!(
    dyn_data_marker: ExemplarCharactersAuxiliary;
    data_marker: LocaleExemplarCharactersAuxiliaryV1;
    func:
    pub fn try_new_auxiliary_unstable();

    /// Get the "auxiliary" set of exemplar characters.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::locale;
    /// use icu::locale::exemplar_chars::ExemplarCharacters;
    ///
    /// let exemplars_auxiliary =
    ///     ExemplarCharacters::try_new_auxiliary(&locale!("en").into())
    ///     .expect("locale should be present");
    ///
    /// assert!(!exemplars_auxiliary.contains('a'));
    /// assert!(!exemplars_auxiliary.contains('z'));
    /// assert!(!exemplars_auxiliary.contains_str("a"));
    /// assert!(exemplars_auxiliary.contains_str("ä"));
    /// assert!(!exemplars_auxiliary.contains_str("ng"));
    /// assert!(!exemplars_auxiliary.contains_str("A"));
    /// ```
    pub fn try_new_auxiliary();
);

make_exemplar_chars_unicode_set_property!(
    dyn_data_marker: ExemplarCharactersPunctuation;
    data_marker: LocaleExemplarCharactersPunctuationV1;
    func:
    pub fn try_new_punctuation_unstable();

    /// Get the "punctuation" set of exemplar characters.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::locale;
    /// use icu::locale::exemplar_chars::ExemplarCharacters;
    ///
    /// let exemplars_punctuation =
    ///     ExemplarCharacters::try_new_punctuation(&locale!("en").into())
    ///     .expect("locale should be present");
    ///
    /// assert!(!exemplars_punctuation.contains('0'));
    /// assert!(!exemplars_punctuation.contains('9'));
    /// assert!(!exemplars_punctuation.contains('%'));
    /// assert!(exemplars_punctuation.contains(','));
    /// assert!(exemplars_punctuation.contains('.'));
    /// assert!(exemplars_punctuation.contains('!'));
    /// assert!(exemplars_punctuation.contains('?'));
    /// ```
    pub fn try_new_punctuation();
);

make_exemplar_chars_unicode_set_property!(
    dyn_data_marker: ExemplarCharactersNumbers;
    data_marker: LocaleExemplarCharactersNumbersV1;
    func:
    pub fn try_new_numbers_unstable();

    /// Get the "numbers" set of exemplar characters.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::locale;
    /// use icu::locale::exemplar_chars::ExemplarCharacters;
    ///
    /// let exemplars_numbers =
    ///     ExemplarCharacters::try_new_numbers(&locale!("en").into())
    ///     .expect("locale should be present");
    ///
    /// assert!(exemplars_numbers.contains('0'));
    /// assert!(exemplars_numbers.contains('9'));
    /// assert!(exemplars_numbers.contains('%'));
    /// assert!(exemplars_numbers.contains(','));
    /// assert!(exemplars_numbers.contains('.'));
    /// assert!(!exemplars_numbers.contains('!'));
    /// assert!(!exemplars_numbers.contains('?'));
    /// ```
    pub fn try_new_numbers();
);

make_exemplar_chars_unicode_set_property!(
    dyn_data_marker: ExemplarCharactersIndex;
    data_marker: LocaleExemplarCharactersIndexV1;
    func:
    pub fn try_new_index_unstable();

    /// Get the "index" set of exemplar characters.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::locale;
    /// use icu::locale::exemplar_chars::ExemplarCharacters;
    ///
    /// let exemplars_index =
    ///     ExemplarCharacters::try_new_index(&locale!("en").into())
    ///     .expect("locale should be present");
    ///
    /// assert!(!exemplars_index.contains('a'));
    /// assert!(!exemplars_index.contains('z'));
    /// assert!(!exemplars_index.contains_str("a"));
    /// assert!(!exemplars_index.contains_str("ä"));
    /// assert!(!exemplars_index.contains_str("ng"));
    /// assert!(exemplars_index.contains_str("A"));
    /// ```
    pub fn try_new_index();
);
