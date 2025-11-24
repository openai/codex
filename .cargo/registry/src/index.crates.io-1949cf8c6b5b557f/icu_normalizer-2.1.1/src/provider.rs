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

// Provider structs must be stable
#![allow(clippy::exhaustive_structs, clippy::exhaustive_enums)]

use icu_collections::char16trie::Char16Trie;
use icu_collections::codepointtrie::CodePointTrie;
use icu_provider::prelude::*;
use zerovec::ZeroVec;

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
    use icu_normalizer_data::*;
    pub mod icu {
        pub use crate as normalizer;
        pub use icu_collections as collections;
    }
    make_provider!(Baked);
    impl_normalizer_nfc_v1!(Baked);
    impl_normalizer_nfd_data_v1!(Baked);
    impl_normalizer_nfd_supplement_v1!(Baked);
    impl_normalizer_nfd_tables_v1!(Baked);
    impl_normalizer_nfkd_data_v1!(Baked);
    impl_normalizer_nfkd_tables_v1!(Baked);
    impl_normalizer_uts46_data_v1!(Baked);
};

icu_provider::data_marker!(
    /// Marker for data for canonical decomposition.
    NormalizerNfdDataV1,
    "normalizer/nfd/data/v1",
    DecompositionData<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// Marker for additional data for canonical decomposition.
    NormalizerNfdTablesV1,
    "normalizer/nfd/tables/v1",
    DecompositionTables<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// Marker for data for compatibility decomposition.
    NormalizerNfkdDataV1,
    "normalizer/nfkd/data/v1",
    DecompositionData<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// Marker for additional data for compatibility decomposition.
    NormalizerNfkdTablesV1,
    "normalizer/nfkd/tables/v1",
    DecompositionTables<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// Marker for data for UTS-46 decomposition.
    NormalizerUts46DataV1,
    "normalizer/uts46/data/v1",
    DecompositionData<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// Marker for data for composition.
    NormalizerNfcV1,
    "normalizer/nfc/v1",
    CanonicalCompositions<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// Marker for additional data for non-recusrsive composition.
    NormalizerNfdSupplementV1,
    "normalizer/nfd/supplement/v1",
    NonRecursiveDecompositionSupplement<'static>,
    is_singleton = true
);

#[cfg(feature = "datagen")]
/// The latest minimum set of markers required by this component.
pub const MARKERS: &[DataMarkerInfo] = &[
    NormalizerNfcV1::INFO,
    NormalizerNfdDataV1::INFO,
    NormalizerNfdTablesV1::INFO,
    NormalizerNfkdDataV1::INFO,
    NormalizerNfkdTablesV1::INFO,
    NormalizerNfdSupplementV1::INFO,
    NormalizerUts46DataV1::INFO,
];

/// Decomposition data
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[derive(Debug, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_normalizer::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct DecompositionData<'data> {
    /// Trie for decomposition.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub trie: CodePointTrie<'data, u32>,
    /// The passthrough bounds of NFD/NFC are lowered to this
    /// maximum instead. (16-bit, because cannot be higher
    /// than 0x0300, which is the bound for NFC.)
    pub passthrough_cap: u16,
}

icu_provider::data_struct!(
    DecompositionData<'_>,
    #[cfg(feature = "datagen")]
);

/// The expansion tables for cases where the decomposition isn't
/// contained in the trie value
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[derive(Debug, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_normalizer::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct DecompositionTables<'data> {
    /// Decompositions that are fully within the BMP
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub scalars16: ZeroVec<'data, u16>,
    /// Decompositions with at least one character outside
    /// the BMP
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub scalars24: ZeroVec<'data, char>,
}

icu_provider::data_struct!(
    DecompositionTables<'_>,
    #[cfg(feature = "datagen")]
);

/// Non-Hangul canonical compositions
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[derive(Debug, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_normalizer::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct CanonicalCompositions<'data> {
    /// Trie keys are two-`char` strings with the second
    /// character coming first. The value, if any, is the
    /// (non-Hangul) canonical composition.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub canonical_compositions: Char16Trie<'data>,
}

icu_provider::data_struct!(
    CanonicalCompositions<'_>,
    #[cfg(feature = "datagen")]
);

/// Non-recursive canonical decompositions that differ from
/// `DecompositionData`.
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[derive(Debug, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_normalizer::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct NonRecursiveDecompositionSupplement<'data> {
    /// Trie for the supplementary non-recursive decompositions
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub trie: CodePointTrie<'data, u32>,
    /// Decompositions with at least one character outside
    /// the BMP
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub scalars24: ZeroVec<'data, char>,
}

icu_provider::data_struct!(
    NonRecursiveDecompositionSupplement<'_>,
    #[cfg(feature = "datagen")]
);
