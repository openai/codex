// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Formatting basic decimal numbers.
//!
//! This module is published as its own crate ([`icu_decimal`](https://docs.rs/icu_decimal/latest/icu_decimal/))
//! and as part of the [`icu`](https://docs.rs/icu/latest/icu/) crate. See the latter for more details on the ICU4X project.
//!
//! Support for currencies, measurement units, and compact notation is planned. To track progress,
//! follow [icu4x#275](https://github.com/unicode-org/icu4x/issues/275).
//!
//! # Examples
//!
//! ## Format a number with Bangla digits
//!
//! ```
//! use icu::decimal::input::Decimal;
//! use icu::decimal::DecimalFormatter;
//! use icu::locale::locale;
//! use writeable::assert_writeable_eq;
//!
//! let formatter =
//!     DecimalFormatter::try_new(locale!("bn").into(), Default::default())
//!         .expect("locale should be present");
//!
//! let decimal = Decimal::from(1000007);
//!
//! assert_writeable_eq!(formatter.format(&decimal), "১০,০০,০০৭");
//! ```
//!
//! ## Format a number with digits after the decimal separator
//!
//! ```
//! use icu::decimal::input::Decimal;
//! use icu::decimal::DecimalFormatter;
//! use icu::locale::Locale;
//! use writeable::assert_writeable_eq;
//!
//! let formatter =
//!     DecimalFormatter::try_new(Default::default(), Default::default())
//!         .expect("locale should be present");
//!
//! let decimal = {
//!     let mut decimal = Decimal::from(200050);
//!     decimal.multiply_pow10(-2);
//!     decimal
//! };
//!
//! assert_writeable_eq!(formatter.format(&decimal), "2,000.50");
//! ```
//!
//! ## Format a number using an alternative numbering system
//!
//! Numbering systems specified in the `-u-nu` subtag will be followed.
//!
//! ```
//! use icu::decimal::input::Decimal;
//! use icu::decimal::DecimalFormatter;
//! use icu::locale::locale;
//! use writeable::assert_writeable_eq;
//!
//! let formatter = DecimalFormatter::try_new(
//!     locale!("th-u-nu-thai").into(),
//!     Default::default(),
//! )
//! .expect("locale should be present");
//!
//! let decimal = Decimal::from(1000007);
//!
//! assert_writeable_eq!(formatter.format(&decimal), "๑,๐๐๐,๐๐๗");
//! ```
//!
//! [`DecimalFormatter`]: DecimalFormatter

// https://github.com/unicode-org/icu4x/blob/main/documents/process/boilerplate.md#library-annotations
#![cfg_attr(not(any(test, doc)), no_std)]
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::exhaustive_structs,
        clippy::exhaustive_enums,
        clippy::trivially_copy_pass_by_ref,
        missing_debug_implementations,
    )
)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod format;
mod grouper;
pub mod options;
pub mod parts;
pub mod provider;
pub(crate) mod size_test_macro;

pub use format::FormattedDecimal;

use fixed_decimal::Decimal;
use icu_locale_core::locale;
use icu_locale_core::preferences::define_preferences;
use icu_provider::prelude::*;
use size_test_macro::size_test;

size_test!(DecimalFormatter, decimal_formatter_size, 96);

define_preferences!(
    /// The preferences for fixed decimal formatting.
    [Copy]
    DecimalFormatterPreferences,
    {
        /// The user's preferred numbering system.
        ///
        /// Corresponds to the `-u-nu` in Unicode Locale Identifier.
        ///
        /// To get the resolved numbering system, you can inspect the data provider.
        /// See the [`provider`] module for an example.
        numbering_system: preferences::NumberingSystem
    }
);

/// Locale preferences used by this crate
pub mod preferences {
    /// **This is a reexport of a type in [`icu::locale`](icu_locale_core::preferences::extensions::unicode::keywords)**.
    #[doc = "\n"] // prevent autoformatting
    pub use icu_locale_core::preferences::extensions::unicode::keywords::NumberingSystem;
}

/// Types that can be fed to [`DecimalFormatter`] and their utilities
///
/// This module contains re-exports from the [`fixed_decimal`] crate.
pub mod input {
    pub use fixed_decimal::Decimal;
    #[cfg(feature = "ryu")]
    pub use fixed_decimal::FloatPrecision;
    pub use fixed_decimal::SignDisplay;
}

/// A formatter for [`Decimal`], rendering decimal digits in an i18n-friendly way.
///
/// [`DecimalFormatter`] supports:
///
/// 1. Rendering in the local numbering system
/// 2. Locale-sensitive grouping separator positions
/// 3. Locale-sensitive plus and minus signs
///
/// To get the resolved numbering system, see [`provider`].
///
/// See the crate-level documentation for examples.
#[doc = decimal_formatter_size!()]
#[derive(Debug, Clone)]
pub struct DecimalFormatter {
    options: options::DecimalFormatterOptions,
    symbols: DataPayload<provider::DecimalSymbolsV1>,
    digits: DataPayload<provider::DecimalDigitsV1>,
}

impl AsRef<DecimalFormatter> for DecimalFormatter {
    fn as_ref(&self) -> &DecimalFormatter {
        self
    }
}

impl DecimalFormatter {
    icu_provider::gen_buffer_data_constructors!(
        (prefs: DecimalFormatterPreferences, options: options::DecimalFormatterOptions) -> error: DataError,
        /// Creates a new [`DecimalFormatter`] from compiled data and an options bag.
    );

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::try_new)]
    pub fn try_new_unstable<
        D: DataProvider<provider::DecimalSymbolsV1> + DataProvider<provider::DecimalDigitsV1> + ?Sized,
    >(
        provider: &D,
        prefs: DecimalFormatterPreferences,
        options: options::DecimalFormatterOptions,
    ) -> Result<Self, DataError> {
        let locale = provider::DecimalSymbolsV1::make_locale(prefs.locale_preferences);
        let provided_nu = prefs.numbering_system.as_ref().map(|s| s.as_str());

        // In case the user explicitly specified a numbering system, use digits from that numbering system. In case of explicitly specified numbering systems,
        // the resolved one may end up being different due to a lack of data or fallback, e.g. attempting to resolve en-u-nu-thai will likely produce en-u-nu-Latn data.
        //
        // This correctly handles the following cases:
        // - Explicitly specified numbering system that is the same as the resolved numbering system: This code effects no change
        // - Explicitly specified numbering system that is different from the resolved one: This code overrides it, but the symbols are still correctly loaded for the locale
        // - No explicitly specified numbering system: The default numbering system for the locale is used.
        // - Explicitly specified numbering system without data for it: this falls back to the resolved numbering system
        //
        // Assuming the provider has symbols for en-u-nu-latn, th-u-nu-thai (default for th), and th-u-nu-latin, this produces the following behavior:
        //
        // | Input Locale | Symbols | Digits | Return value of `numbering_system()` |
        // |--------------|---------|--------|--------------------------------------|
        // | en           | latn    | latn   | latn                                 |
        // | en-u-nu-thai | latn    | thai   | thai                                 |
        // | th           | thai    | thai   | thai                                 |
        // | th-u-nu-latn | latn    | latn   | latn                                 |
        // | en-u-nu-wxyz | latn    | latn   | latn                                 |
        // | th-u-nu-wxyz | thai    | thai   | thai                                 |

        if let Some(provided_nu) = provided_nu {
            // Load symbols for the locale/numsys pair provided
            let symbols: DataPayload<provider::DecimalSymbolsV1> = provider
                .load(DataRequest {
                    id: DataIdentifierBorrowed::for_marker_attributes_and_locale(
                        DataMarkerAttributes::from_str_or_panic(provided_nu),
                        &locale,
                    ),
                    ..Default::default()
                })
                // If it doesn't exist, fall back to the locale
                .or_else(|_err| {
                    provider.load(DataRequest {
                        id: DataIdentifierBorrowed::for_marker_attributes_and_locale(
                            DataMarkerAttributes::empty(),
                            &locale,
                        ),
                        ..Default::default()
                    })
                })?
                .payload;

            let resolved_nu = symbols.get().numsys();

            // Attempt to load the provided numbering system first
            let digits = provider
                .load(DataRequest {
                    id: DataIdentifierBorrowed::for_marker_attributes_and_locale(
                        DataMarkerAttributes::from_str_or_panic(provided_nu),
                        &locale!("und").into(),
                    ),
                    ..Default::default()
                })
                .or_else(|_err| {
                    provider.load(DataRequest {
                        id: DataIdentifierBorrowed::for_marker_attributes_and_locale(
                            DataMarkerAttributes::from_str_or_panic(resolved_nu),
                            &locale!("und").into(),
                        ),
                        ..Default::default()
                    })
                })?
                .payload;
            Ok(Self {
                options,
                symbols,
                digits,
            })
        } else {
            let symbols: DataPayload<provider::DecimalSymbolsV1> = provider
                .load(DataRequest {
                    id: DataIdentifierBorrowed::for_marker_attributes_and_locale(
                        DataMarkerAttributes::empty(),
                        &locale,
                    ),
                    ..Default::default()
                })?
                .payload;

            let resolved_nu = symbols.get().numsys();

            let digits = provider
                .load(DataRequest {
                    id: DataIdentifierBorrowed::for_marker_attributes_and_locale(
                        DataMarkerAttributes::from_str_or_panic(resolved_nu),
                        &locale!("und").into(),
                    ),
                    ..Default::default()
                })?
                .payload;
            Ok(Self {
                options,
                symbols,
                digits,
            })
        }
    }

    /// Formats a [`Decimal`], returning a [`FormattedDecimal`].
    pub fn format<'l>(&'l self, value: &'l Decimal) -> FormattedDecimal<'l> {
        FormattedDecimal {
            value,
            options: &self.options,
            symbols: self.symbols.get(),
            digits: self.digits.get(),
        }
    }

    /// Formats a [`Decimal`], returning a [`String`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn format_to_string(&self, value: &Decimal) -> alloc::string::String {
        use writeable::Writeable;
        self.format(value).write_to_string().into_owned()
    }
}

#[test]
fn test_numbering_resolution_fallback() {
    fn test_locale(locale: icu_locale_core::Locale, expected_format: &str) {
        let formatter =
            DecimalFormatter::try_new((&locale).into(), Default::default()).expect("Must load");
        let fd = 1234.into();
        writeable::assert_writeable_eq!(
            formatter.format(&fd),
            expected_format,
            "Correct format for {locale}"
        );
    }

    // Loading en with default latn numsys
    test_locale(locale!("en"), "1,234");
    // Loading en with arab numsys not present in symbols data will mix en symbols with arab digits
    test_locale(locale!("en-u-nu-arab"), "١,٢٣٤");
    // Loading ar-EG with default (arab) numsys
    test_locale(locale!("ar-EG"), "١٬٢٣٤");
    // Loading ar-EG with overridden latn numsys, present in symbols data, uses ar-EG-u-nu-latn symbols data
    test_locale(locale!("ar-EG-u-nu-latn"), "1,234");
    // Loading ar-EG with overridden thai numsys, not present in symbols data, uses ar-EG symbols data + thai digits
    test_locale(locale!("ar-EG-u-nu-thai"), "๑٬๒๓๔");
    // Loading with nonexistant numbering systems falls back to default
    test_locale(locale!("en-u-nu-wxyz"), "1,234");
    test_locale(locale!("ar-EG-u-nu-wxyz"), "١٬٢٣٤");
}
