// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! 🚧 \[Unstable\] Data provider struct definitions for this ICU4X component.
//!
//! <div class="stab unstable">
//! 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
//! including in SemVer minor releases. While the serde representation of data structs is guaranteed
//! to be stable, their Rust representation might not be. Use with caution.
//! </div>
//!
//! Read more about data providers: [`icu_provider`]
//!
//! # Examples
//!
//! ## Get the resolved numbering system
//!
//! In a constructor call, the _last_ request for [`DecimalDigitsV1`]
//! contains the resolved numbering system as its attribute:
//!
//! ```
//! use icu::decimal::provider::DecimalDigitsV1;
//! use icu::decimal::DecimalFormatter;
//! use icu::locale::locale;
//! use icu_provider::prelude::*;
//! use std::any::TypeId;
//! use std::cell::RefCell;
//!
//! struct NumberingSystemInspectionProvider<P> {
//!     inner: P,
//!     numbering_system: RefCell<Option<Box<DataMarkerAttributes>>>,
//! }
//!
//! impl<M, P> DataProvider<M> for NumberingSystemInspectionProvider<P>
//! where
//!     M: DataMarker,
//!     P: DataProvider<M>,
//! {
//!     fn load(&self, req: DataRequest) -> Result<DataResponse<M>, DataError> {
//!         if TypeId::of::<M>() == TypeId::of::<DecimalDigitsV1>() {
//!             *self.numbering_system.try_borrow_mut().unwrap() =
//!                 Some(req.id.marker_attributes.to_owned());
//!         }
//!         self.inner.load(req)
//!     }
//! }
//!
//! let provider = NumberingSystemInspectionProvider {
//!     inner: icu::decimal::provider::Baked,
//!     numbering_system: RefCell::new(None),
//! };
//!
//! let formatter = DecimalFormatter::try_new_unstable(
//!     &provider,
//!     locale!("th").into(),
//!     Default::default(),
//! )
//! .unwrap();
//!
//! assert_eq!(
//!     provider
//!         .numbering_system
//!         .borrow()
//!         .as_ref()
//!         .map(|x| x.as_str()),
//!     Some("latn")
//! );
//!
//! let formatter = DecimalFormatter::try_new_unstable(
//!     &provider,
//!     locale!("th-u-nu-thai").into(),
//!     Default::default(),
//! )
//! .unwrap();
//!
//! assert_eq!(
//!     provider
//!         .numbering_system
//!         .borrow()
//!         .as_ref()
//!         .map(|x| x.as_str()),
//!     Some("thai")
//! );
//!
//! let formatter = DecimalFormatter::try_new_unstable(
//!     &provider,
//!     locale!("th-u-nu-adlm").into(),
//!     Default::default(),
//! )
//! .unwrap();
//!
//! assert_eq!(
//!     provider
//!         .numbering_system
//!         .borrow()
//!         .as_ref()
//!         .map(|x| x.as_str()),
//!     Some("adlm")
//! );
//! ```

// Provider structs must be stable
#![allow(clippy::exhaustive_structs)]
#![allow(clippy::exhaustive_enums)]

use icu_provider::prelude::*;
use zerovec::VarZeroCow;

#[cfg(feature = "compiled_data")]
#[derive(Debug)]
/// Baked data
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. In particular, the `DataProvider` implementations are only
/// guaranteed to match with this version's `*_unstable` providers. Use with caution.
/// </div>
pub struct Baked;

#[cfg(feature = "compiled_data")]
#[allow(unused_imports)]
const _: () = {
    use icu_decimal_data::*;
    pub mod icu {
        pub use crate as decimal;
        pub use icu_locale as locale;
    }
    make_provider!(Baked);
    impl_decimal_symbols_v1!(Baked);
    impl_decimal_digits_v1!(Baked);
};

icu_provider::data_marker!(
    /// Data marker for decimal symbols
    DecimalSymbolsV1,
    "decimal/symbols/v1",
    DecimalSymbols<'static>,
);

icu_provider::data_marker!(
    /// The digits for a given numbering system. This data ought to be stored in the `und` locale with an auxiliary key
    /// set to the numbering system code.
    DecimalDigitsV1,
    "decimal/digits/v1",
    [char; 10],
    #[cfg(feature = "datagen")]
    attributes_domain = "numbering_system"
);

#[cfg(feature = "datagen")]
/// The latest minimum set of markers required by this component.
pub const MARKERS: &[DataMarkerInfo] = &[DecimalSymbolsV1::INFO, DecimalDigitsV1::INFO];

/// A collection of settings expressing where to put grouping separators in a decimal number.
/// For example, `1,000,000` has two grouping separators, positioned along every 3 digits.
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[derive(Debug, PartialEq, Clone, yoke::Yokeable, Copy, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_decimal::provider))]
pub struct GroupingSizes {
    /// The size of the first (lowest-magnitude) group.
    ///
    /// If 0, grouping separators will never be shown.
    pub primary: u8,

    /// The size of groups after the first group.
    ///
    /// If 0, defaults to be the same as `primary`.
    pub secondary: u8,

    /// The minimum number of digits required before the first group. For example, if `primary=3`
    /// and `min_grouping=2`, grouping separators will be present on 10,000 and above.
    pub min_grouping: u8,
}

/// A stack representation of the strings used in [`DecimalSymbols`], i.e. a builder type
/// for [`DecimalSymbolsStrs`]. This type can be obtained from a [`DecimalSymbolsStrs`]
/// the `From`/`Into` traits.
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[derive(Debug, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "datagen", derive(serde::Serialize))]
#[zerovec::make_varule(DecimalSymbolsStrs)]
#[zerovec::derive(Debug)]
#[zerovec::skip_derive(Ord)]
#[cfg_attr(not(feature = "alloc"), zerovec::skip_derive(ZeroMapKV, ToOwned))]
#[cfg_attr(feature = "serde", zerovec::derive(Deserialize))]
#[cfg_attr(feature = "datagen", zerovec::derive(Serialize))]
// Each affix/separator is at most three characters, which tends to be around 3-12 bytes each
// and the numbering system is at most 8 ascii bytes, All put together the indexing is extremely
// unlikely to have to go past 256.
#[zerovec::format(zerovec::vecs::Index8)]
pub struct DecimalSymbolStrsBuilder<'data> {
    /// Prefix to apply when a negative sign is needed.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub minus_sign_prefix: VarZeroCow<'data, str>,
    /// Suffix to apply when a negative sign is needed.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub minus_sign_suffix: VarZeroCow<'data, str>,

    /// Prefix to apply when a positive sign is needed.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub plus_sign_prefix: VarZeroCow<'data, str>,
    /// Suffix to apply when a positive sign is needed.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub plus_sign_suffix: VarZeroCow<'data, str>,

    /// Character used to separate the integer and fraction parts of the number.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub decimal_separator: VarZeroCow<'data, str>,

    /// Character used to separate groups in the integer part of the number.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub grouping_separator: VarZeroCow<'data, str>,

    /// The numbering system to use.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub numsys: VarZeroCow<'data, str>,
}

#[cfg(feature = "alloc")]
impl DecimalSymbolStrsBuilder<'_> {
    /// Build a [`DecimalSymbolsStrs`]
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    pub fn build(&self) -> VarZeroCow<'static, DecimalSymbolsStrs> {
        VarZeroCow::from_encodeable(self)
    }
}

/// Symbols and metadata required for formatting a [`Decimal`](crate::Decimal).
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[derive(Debug, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_decimal::provider))]
pub struct DecimalSymbols<'data> {
    /// String data for the symbols: +/- affixes and separators
    #[cfg_attr(feature = "serde", serde(borrow))]
    // We use a VarZeroCow here to reduce the stack size of DecimalSymbols: instead of serializing multiple strs,
    // this type will now serialize as a single u8 buffer with optimized indexing that packs all the data together
    pub strings: VarZeroCow<'data, DecimalSymbolsStrs>,

    /// Settings used to determine where to place groups in the integer part of the number.
    pub grouping_sizes: GroupingSizes,
}

icu_provider::data_struct!(
    DecimalSymbols<'_>,
    #[cfg(feature = "datagen")]
);

impl DecimalSymbols<'_> {
    /// Return (prefix, suffix) for the minus sign
    pub fn minus_sign_affixes(&self) -> (&str, &str) {
        (
            self.strings.minus_sign_prefix(),
            self.strings.minus_sign_suffix(),
        )
    }
    /// Return (prefix, suffix) for the minus sign
    pub fn plus_sign_affixes(&self) -> (&str, &str) {
        (
            self.strings.plus_sign_prefix(),
            self.strings.plus_sign_suffix(),
        )
    }
    /// Return thhe decimal separator
    pub fn decimal_separator(&self) -> &str {
        self.strings.decimal_separator()
    }
    /// Return thhe decimal separator
    pub fn grouping_separator(&self) -> &str {
        self.strings.grouping_separator()
    }

    /// Return the numbering system
    pub fn numsys(&self) -> &str {
        self.strings.numsys()
    }
}

impl DecimalSymbols<'static> {
    /// Create a new en-US format for use in testing
    #[cfg(feature = "datagen")]
    pub fn new_en_for_testing() -> Self {
        let strings = DecimalSymbolStrsBuilder {
            minus_sign_prefix: VarZeroCow::new_borrowed("-"),
            minus_sign_suffix: VarZeroCow::new_borrowed(""),
            plus_sign_prefix: VarZeroCow::new_borrowed("+"),
            plus_sign_suffix: VarZeroCow::new_borrowed(""),
            decimal_separator: VarZeroCow::new_borrowed("."),
            grouping_separator: VarZeroCow::new_borrowed(","),
            numsys: VarZeroCow::new_borrowed("latn"),
        };
        Self {
            strings: VarZeroCow::from_encodeable(&strings),
            grouping_sizes: GroupingSizes {
                primary: 3,
                secondary: 3,
                min_grouping: 1,
            },
        }
    }
}
