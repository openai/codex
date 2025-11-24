// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

// Provider structs must be stable
#![allow(clippy::exhaustive_structs, clippy::exhaustive_enums)]

//! 🚧 \[Unstable\] Data provider struct definitions for this ICU4X component.
//!
//! <div class="stab unstable">
//! 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
//! including in SemVer minor releases. While the serde representation of data structs is guaranteed
//! to be stable, their Rust representation might not be. Use with caution.
//! </div>
//!
//! Read more about data providers: [`icu_provider`]

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
    use icu_locale_data::*;
    pub mod icu {
        pub use crate as locale;
        pub use icu_collections as collections;
    }
    make_provider!(Baked);
    impl_locale_aliases_v1!(Baked);
    impl_locale_likely_subtags_extended_v1!(Baked);
    impl_locale_likely_subtags_language_v1!(Baked);
    impl_locale_likely_subtags_script_region_v1!(Baked);
    impl_locale_parents_v1!(Baked);
    impl_locale_script_direction_v1!(Baked);

    impl_locale_exemplar_characters_auxiliary_v1!(Baked);
    impl_locale_exemplar_characters_index_v1!(Baked);
    impl_locale_exemplar_characters_main_v1!(Baked);
    impl_locale_exemplar_characters_numbers_v1!(Baked);
    impl_locale_exemplar_characters_punctuation_v1!(Baked);
};

icu_provider::data_marker!(
    /// Marker for locale alias data.
    LocaleAliasesV1,
    "locale/aliases/v1",
    Aliases<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// Marker for data for likely subtags for languages.
    LocaleLikelySubtagsLanguageV1,
    "locale/likely/subtags/language/v1",
    LikelySubtagsForLanguage<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// Marker for data for likely subtags for scripts and regions.
    LocaleLikelySubtagsScriptRegionV1,
    "locale/likely/subtags/script/region/v1",
    LikelySubtagsForScriptRegion<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// Marker for extended data for likely subtags.
    LocaleLikelySubtagsExtendedV1,
    "locale/likely/subtags/extended/v1",
    LikelySubtagsExtended<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// Marker for locale fallback parents data.
    LocaleParentsV1,
    "locale/parents/v1",
    Parents<'static>,
    is_singleton = true
);

icu_provider::data_marker!(
    /// Marker for script direction data.
    LocaleScriptDirectionV1,
    "locale/script/direction/v1",
    ScriptDirection<'static>,
    is_singleton = true
);

icu_provider::data_marker!(
    /// Marker for auxiliary exemplar characters data.
    LocaleExemplarCharactersAuxiliaryV1,
    "locale/exemplar/characters/auxiliary/v1",
    ExemplarCharactersData<'static>,
);
icu_provider::data_marker!(
    /// Marker for index exemplar characters data.
    LocaleExemplarCharactersIndexV1,
    "locale/exemplar/characters/index/v1",
    ExemplarCharactersData<'static>,
);
icu_provider::data_marker!(
    /// Marker for main exemplar characters data.
    LocaleExemplarCharactersMainV1,
    "locale/exemplar/characters/main/v1",
    ExemplarCharactersData<'static>,
);
icu_provider::data_marker!(
    /// Marker for numbers exemplar characters data.
    LocaleExemplarCharactersNumbersV1,
    "locale/exemplar/characters/numbers/v1",
    ExemplarCharactersData<'static>,
);
icu_provider::data_marker!(
    /// Marker for punctuation exemplar characters data.
    LocaleExemplarCharactersPunctuationV1,
    "locale/exemplar/characters/punctuation/v1",
    ExemplarCharactersData<'static>,
);

#[cfg(feature = "datagen")]
/// The latest minimum set of markers required by this component.
pub const MARKERS: &[DataMarkerInfo] = &[
    LocaleAliasesV1::INFO,
    LocaleExemplarCharactersAuxiliaryV1::INFO,
    LocaleExemplarCharactersIndexV1::INFO,
    LocaleExemplarCharactersMainV1::INFO,
    LocaleExemplarCharactersNumbersV1::INFO,
    LocaleExemplarCharactersPunctuationV1::INFO,
    LocaleLikelySubtagsExtendedV1::INFO,
    LocaleLikelySubtagsLanguageV1::INFO,
    LocaleLikelySubtagsScriptRegionV1::INFO,
    LocaleParentsV1::INFO,
    LocaleScriptDirectionV1::INFO,
];

use alloc::borrow::Cow;
use icu_collections::codepointinvliststringlist::CodePointInversionListAndStringList;
use icu_locale_core::subtags::{Language, Region, Script, Variant};
use icu_provider::prelude::*;
use potential_utf::PotentialUtf8;
use tinystr::{TinyAsciiStr, UnvalidatedTinyAsciiStr};
use zerovec::{VarZeroVec, ZeroMap, ZeroSlice, ZeroVec};

// We use raw TinyAsciiStrs for map keys, as we then don't have to
// validate them as subtags on deserialization. Map lookup can be
// done even if they are not valid tags (an invalid key will just
// become inaccessible).
type UnvalidatedLanguage = UnvalidatedTinyAsciiStr<3>;
type UnvalidatedScript = UnvalidatedTinyAsciiStr<4>;
type UnvalidatedRegion = UnvalidatedTinyAsciiStr<3>;
type UnvalidatedVariant = UnvalidatedTinyAsciiStr<8>;
type UnvalidatedSubdivision = UnvalidatedTinyAsciiStr<7>;
type SemivalidatedSubdivision = TinyAsciiStr<7>;

// LanguageIdentifier doesn't have an AsULE implementation, so we have
// to store strs and parse when needed.
type UnvalidatedLanguageIdentifier = str;
type UnvalidatedLanguageIdentifierPair = StrStrPairVarULE;
type UnvalidatedLanguageVariantsPair = LanguageStrStrPairVarULE;

#[zerovec::make_varule(StrStrPairVarULE)]
#[zerovec::derive(Debug)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Deserialize),
    zerovec::derive(Deserialize)
)]
#[cfg_attr(
    feature = "datagen",
    derive(serde::Serialize, databake::Bake),
    zerovec::derive(Serialize),
    databake(path = icu_locale::provider),
)]
/// A pair of strings with a EncodeAsVarULE implementation.
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
pub struct StrStrPair<'a>(
    #[cfg_attr(feature = "serde", serde(borrow))] pub Cow<'a, str>,
    #[cfg_attr(feature = "serde", serde(borrow))] pub Cow<'a, str>,
);

#[zerovec::make_varule(LanguageStrStrPairVarULE)]
#[zerovec::derive(Debug)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Deserialize),
    zerovec::derive(Deserialize)
)]
#[cfg_attr(
    feature = "datagen",
    derive(serde::Serialize, databake::Bake),
    zerovec::derive(Serialize),
    databake(path = icu_locale::provider),
)]
/// A triplet of strings with a EncodeAsVarULE implementation.
pub struct LanguageStrStrPair<'a>(
    pub Language,
    #[cfg_attr(feature = "serde", serde(borrow))] pub Cow<'a, str>,
    #[cfg_attr(feature = "serde", serde(borrow))] pub Cow<'a, str>,
);

#[derive(PartialEq, Clone, Default, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_locale::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[yoke(prove_covariance_manually)]
/// This alias data is used for locale canonicalization.
///
/// Each field defines a
/// mapping from an old identifier to a new identifier, based upon the rules in
/// from <http://unicode.org/reports/tr35/#LocaleId_Canonicalization>. The data
/// is stored in sorted order, allowing for binary search to identify rules to
/// apply. It is broken down into smaller vectors based upon some characteristic
/// of the data, to help avoid unnecessary searches. For example, the `sgn_region`
/// field contains aliases for sign language and region, so that it is not
/// necessary to search the data unless the input is a sign language.
///
/// The algorithm in tr35 is not guaranteed to terminate on data other than what
/// is currently in CLDR. For this reason, it is not a good idea to attempt to add
/// or modify aliases for use in this structure.
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
// TODO: Use validated types as value types
// Notice:  improves the alignment of `language_variants` speeding up canonicalization by upon
// to 40%. See https://github.com/unicode-org/icu4x/pull/2935 for details.
#[derive(Debug)]
pub struct Aliases<'data> {
    /// `[language, variant(-variant)*] -> [langid]`
    /// This is not a map as it's searched linearly according to the canonicalization rules.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub language_variants: VarZeroVec<'data, UnvalidatedLanguageVariantsPair>,
    /// `sgn-[region] -> [language]`
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub sgn_region: ZeroMap<'data, UnvalidatedRegion, Language>,
    /// `[language{2}] -> [langid]`
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub language_len2: ZeroMap<'data, UnvalidatedTinyAsciiStr<2>, UnvalidatedLanguageIdentifier>,
    /// `[language{3}] -> [langid]`
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub language_len3: ZeroMap<'data, UnvalidatedLanguage, UnvalidatedLanguageIdentifier>,
    /// `[langid] -> [langid]`
    /// This is not a map as it's searched linearly according to the canonicalization rules.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub language: VarZeroVec<'data, UnvalidatedLanguageIdentifierPair>,

    /// `[script] -> [script]`
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub script: ZeroMap<'data, UnvalidatedScript, Script>,

    /// `[region{2}] -> [region]`
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub region_alpha: ZeroMap<'data, UnvalidatedTinyAsciiStr<2>, Region>,
    /// `[region{3}] -> [region]`
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub region_num: ZeroMap<'data, UnvalidatedRegion, Region>,

    /// `[region] -> [region]+`
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub complex_region: ZeroMap<'data, UnvalidatedRegion, ZeroSlice<Region>>,

    /// `[variant] -> [variant]`
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub variant: ZeroMap<'data, UnvalidatedVariant, Variant>,

    /// `[value{7}] -> [value{7}]`
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub subdivision: ZeroMap<'data, UnvalidatedSubdivision, SemivalidatedSubdivision>,
}

icu_provider::data_struct!(
    Aliases<'_>,
    #[cfg(feature = "datagen")]
);

#[derive(Debug, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_locale::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
/// This likely subtags data is used for the minimize and maximize operations.
///
/// Each field defines a mapping from an old identifier to a new identifier,
/// based upon the rules in
/// <https://www.unicode.org/reports/tr35/#Likely_Subtags>.
///
/// The data is stored is broken down into smaller vectors based upon the rules
/// defined for the likely subtags maximize algorithm.
///
/// For efficiency, only the relevant part of the LanguageIdentifier is stored
/// for searching and replacing. E.g., the `language_script` field is used to store
/// rules for `LanguageIdentifier`s that contain a language and a script, but not a
/// region.
///
/// This struct contains mappings when the input contains a language subtag.
/// Also see [`LikelySubtagsForScriptRegion`].
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[yoke(prove_covariance_manually)]
pub struct LikelySubtagsForLanguage<'data> {
    /// Language and script.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub language_script: ZeroMap<'data, (UnvalidatedLanguage, UnvalidatedScript), Region>,
    /// Language and region.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub language_region: ZeroMap<'data, (UnvalidatedLanguage, UnvalidatedRegion), Script>,
    /// Just language.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub language: ZeroMap<'data, UnvalidatedLanguage, (Script, Region)>,
    /// Undefined.
    pub und: (Language, Script, Region),
}

icu_provider::data_struct!(
    LikelySubtagsForLanguage<'_>,
    #[cfg(feature = "datagen")]
);

#[derive(Debug, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_locale::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
/// This likely subtags data is used for the minimize and maximize operations.
///
/// Each field defines a mapping from an old identifier to a new identifier,
/// based upon the rules in
/// <https://www.unicode.org/reports/tr35/#Likely_Subtags>.
///
/// The data is stored is broken down into smaller vectors based upon the rules
/// defined for the likely subtags maximize algorithm.
///
/// For efficiency, only the relevant part of the LanguageIdentifier is stored
/// for searching and replacing. E.g., the `script_region` field is used to store
/// rules for `LanguageIdentifier`s that contain a script and a region, but not a
/// language.
///
/// This struct contains mappings when the input does not contain a language subtag.
/// Also see [`LikelySubtagsForLanguage`].
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[yoke(prove_covariance_manually)]
pub struct LikelySubtagsForScriptRegion<'data> {
    /// Script and region.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub script_region: ZeroMap<'data, (UnvalidatedScript, UnvalidatedRegion), Language>,
    /// Just script.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub script: ZeroMap<'data, UnvalidatedScript, (Language, Region)>,
    /// Just region.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub region: ZeroMap<'data, UnvalidatedRegion, (Language, Script)>,
}

icu_provider::data_struct!(
    LikelySubtagsForScriptRegion<'_>,
    #[cfg(feature = "datagen")]
);

#[derive(Debug, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_locale::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
/// This likely subtags data is used for full coverage of locales, including ones that
/// don't otherwise have data in the Common Locale Data Repository (CLDR).
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[yoke(prove_covariance_manually)]
pub struct LikelySubtagsExtended<'data> {
    /// Language and script.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub language_script: ZeroMap<'data, (UnvalidatedLanguage, UnvalidatedScript), Region>,
    /// Language and region.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub language_region: ZeroMap<'data, (UnvalidatedLanguage, UnvalidatedRegion), Script>,
    /// Just language.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub language: ZeroMap<'data, UnvalidatedLanguage, (Script, Region)>,
    /// Script and region.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub script_region: ZeroMap<'data, (UnvalidatedScript, UnvalidatedRegion), Language>,
    /// Just script.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub script: ZeroMap<'data, UnvalidatedScript, (Language, Region)>,
    /// Just region.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub region: ZeroMap<'data, UnvalidatedRegion, (Language, Script)>,
}

icu_provider::data_struct!(
    LikelySubtagsExtended<'_>,
    #[cfg(feature = "datagen")]
);

/// Locale fallback rules derived from CLDR parent locales data.
#[derive(Default, Clone, PartialEq, Debug, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_locale::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[yoke(prove_covariance_manually)]
pub struct Parents<'data> {
    /// Map from language identifier to language identifier, indicating that the language on the
    /// left should inherit from the language on the right.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub parents: ZeroMap<'data, PotentialUtf8, (Language, Option<Script>, Option<Region>)>,
}

icu_provider::data_struct!(
    Parents<'_>,
    #[cfg(feature = "datagen")]
);

#[derive(Debug, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_locale::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
/// This directionality data is used to determine the script directionality of a locale.
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[yoke(prove_covariance_manually)]
pub struct ScriptDirection<'data> {
    /// Scripts in right-to-left direction.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub rtl: ZeroVec<'data, UnvalidatedScript>,
    /// Scripts in left-to-right direction.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub ltr: ZeroVec<'data, UnvalidatedScript>,
}

icu_provider::data_struct!(
    ScriptDirection<'_>,
    #[cfg(feature = "datagen")]
);

/// A set of characters and strings which share a particular property value.
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[derive(Debug, Eq, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(
    feature = "datagen", 
    derive(serde::Serialize, databake::Bake),
    databake(path = icu_locale::provider),
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct ExemplarCharactersData<'data>(
    #[cfg_attr(feature = "serde", serde(borrow))] pub CodePointInversionListAndStringList<'data>,
);

icu_provider::data_struct!(
    ExemplarCharactersData<'_>,
    #[cfg(feature = "datagen")]
);
