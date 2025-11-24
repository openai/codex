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

pub mod names;

#[cfg(feature = "alloc")]
pub use names::{
    PropertyNameLongCanonicalCombiningClassV1, PropertyNameShortCanonicalCombiningClassV1,
};

pub use names::{
    PropertyNameLongBidiClassV1, PropertyNameLongEastAsianWidthV1,
    PropertyNameLongGeneralCategoryV1, PropertyNameLongGraphemeClusterBreakV1,
    PropertyNameLongHangulSyllableTypeV1, PropertyNameLongIndicSyllabicCategoryV1,
    PropertyNameLongJoiningTypeV1, PropertyNameLongLineBreakV1, PropertyNameLongScriptV1,
    PropertyNameLongSentenceBreakV1, PropertyNameLongVerticalOrientationV1,
    PropertyNameLongWordBreakV1, PropertyNameParseBidiClassV1,
    PropertyNameParseCanonicalCombiningClassV1, PropertyNameParseEastAsianWidthV1,
    PropertyNameParseGeneralCategoryMaskV1, PropertyNameParseGeneralCategoryV1,
    PropertyNameParseGraphemeClusterBreakV1, PropertyNameParseHangulSyllableTypeV1,
    PropertyNameParseIndicSyllabicCategoryV1, PropertyNameParseJoiningTypeV1,
    PropertyNameParseLineBreakV1, PropertyNameParseScriptV1, PropertyNameParseSentenceBreakV1,
    PropertyNameParseVerticalOrientationV1, PropertyNameParseWordBreakV1,
    PropertyNameShortBidiClassV1, PropertyNameShortEastAsianWidthV1,
    PropertyNameShortGeneralCategoryV1, PropertyNameShortGraphemeClusterBreakV1,
    PropertyNameShortHangulSyllableTypeV1, PropertyNameShortIndicSyllabicCategoryV1,
    PropertyNameShortJoiningTypeV1, PropertyNameShortLineBreakV1, PropertyNameShortScriptV1,
    PropertyNameShortSentenceBreakV1, PropertyNameShortVerticalOrientationV1,
    PropertyNameShortWordBreakV1,
};

pub use crate::props::gc::GeneralCategoryULE;
use crate::props::*;
use crate::script::ScriptWithExt;
use core::ops::RangeInclusive;
use icu_collections::codepointinvlist::CodePointInversionList;
use icu_collections::codepointinvliststringlist::CodePointInversionListAndStringList;
use icu_collections::codepointtrie::{CodePointMapRange, CodePointTrie, TrieValue};
use icu_provider::prelude::*;
use zerofrom::ZeroFrom;
use zerovec::{VarZeroVec, ZeroSlice};

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
    use icu_properties_data::*;
    pub mod icu {
        pub use crate as properties;
        pub use icu_collections as collections;
    }
    make_provider!(Baked);
    impl_property_binary_alnum_v1!(Baked);
    impl_property_binary_alphabetic_v1!(Baked);
    impl_property_binary_ascii_hex_digit_v1!(Baked);
    impl_property_binary_basic_emoji_v1!(Baked);
    impl_property_binary_bidi_control_v1!(Baked);
    impl_property_binary_bidi_mirrored_v1!(Baked);
    impl_property_binary_blank_v1!(Baked);
    impl_property_binary_case_ignorable_v1!(Baked);
    impl_property_binary_case_sensitive_v1!(Baked);
    impl_property_binary_cased_v1!(Baked);
    impl_property_binary_changes_when_casefolded_v1!(Baked);
    impl_property_binary_changes_when_casemapped_v1!(Baked);
    impl_property_binary_changes_when_lowercased_v1!(Baked);
    impl_property_binary_changes_when_nfkc_casefolded_v1!(Baked);
    impl_property_binary_changes_when_titlecased_v1!(Baked);
    impl_property_binary_changes_when_uppercased_v1!(Baked);
    impl_property_binary_dash_v1!(Baked);
    impl_property_binary_default_ignorable_code_point_v1!(Baked);
    impl_property_binary_deprecated_v1!(Baked);
    impl_property_binary_diacritic_v1!(Baked);
    impl_property_binary_emoji_component_v1!(Baked);
    impl_property_binary_emoji_modifier_base_v1!(Baked);
    impl_property_binary_emoji_modifier_v1!(Baked);
    impl_property_binary_emoji_presentation_v1!(Baked);
    impl_property_binary_emoji_v1!(Baked);
    impl_property_binary_extended_pictographic_v1!(Baked);
    impl_property_binary_extender_v1!(Baked);
    impl_property_binary_full_composition_exclusion_v1!(Baked);
    impl_property_binary_graph_v1!(Baked);
    impl_property_binary_grapheme_base_v1!(Baked);
    impl_property_binary_grapheme_extend_v1!(Baked);
    impl_property_binary_grapheme_link_v1!(Baked);
    impl_property_binary_hex_digit_v1!(Baked);
    impl_property_binary_hyphen_v1!(Baked);
    impl_property_binary_id_compat_math_continue_v1!(Baked);
    impl_property_binary_id_compat_math_start_v1!(Baked);
    impl_property_binary_id_continue_v1!(Baked);
    impl_property_binary_id_start_v1!(Baked);
    impl_property_binary_ideographic_v1!(Baked);
    impl_property_binary_ids_binary_operator_v1!(Baked);
    impl_property_binary_ids_trinary_operator_v1!(Baked);
    impl_property_binary_ids_unary_operator_v1!(Baked);
    impl_property_binary_join_control_v1!(Baked);
    impl_property_binary_logical_order_exception_v1!(Baked);
    impl_property_binary_lowercase_v1!(Baked);
    impl_property_binary_math_v1!(Baked);
    impl_property_binary_modifier_combining_mark_v1!(Baked);
    impl_property_binary_nfc_inert_v1!(Baked);
    impl_property_binary_nfd_inert_v1!(Baked);
    impl_property_binary_nfkc_inert_v1!(Baked);
    impl_property_binary_nfkd_inert_v1!(Baked);
    impl_property_binary_noncharacter_code_point_v1!(Baked);
    impl_property_binary_pattern_syntax_v1!(Baked);
    impl_property_binary_pattern_white_space_v1!(Baked);
    impl_property_binary_prepended_concatenation_mark_v1!(Baked);
    impl_property_binary_print_v1!(Baked);
    impl_property_binary_quotation_mark_v1!(Baked);
    impl_property_binary_radical_v1!(Baked);
    impl_property_binary_regional_indicator_v1!(Baked);
    impl_property_binary_segment_starter_v1!(Baked);
    impl_property_binary_sentence_terminal_v1!(Baked);
    impl_property_binary_soft_dotted_v1!(Baked);
    impl_property_binary_terminal_punctuation_v1!(Baked);
    impl_property_binary_unified_ideograph_v1!(Baked);
    impl_property_binary_uppercase_v1!(Baked);
    impl_property_binary_variation_selector_v1!(Baked);
    impl_property_binary_white_space_v1!(Baked);
    impl_property_binary_xdigit_v1!(Baked);
    impl_property_binary_xid_continue_v1!(Baked);
    impl_property_binary_xid_start_v1!(Baked);
    impl_property_enum_bidi_class_v1!(Baked);
    impl_property_enum_bidi_mirroring_glyph_v1!(Baked);
    impl_property_enum_canonical_combining_class_v1!(Baked);
    impl_property_enum_east_asian_width_v1!(Baked);
    impl_property_enum_general_category_v1!(Baked);
    impl_property_enum_grapheme_cluster_break_v1!(Baked);
    impl_property_enum_hangul_syllable_type_v1!(Baked);
    impl_property_enum_indic_conjunct_break_v1!(Baked);
    impl_property_enum_indic_syllabic_category_v1!(Baked);
    impl_property_enum_joining_type_v1!(Baked);
    impl_property_enum_line_break_v1!(Baked);
    impl_property_enum_script_v1!(Baked);
    impl_property_enum_sentence_break_v1!(Baked);
    impl_property_enum_vertical_orientation_v1!(Baked);
    impl_property_enum_word_break_v1!(Baked);
    impl_property_name_long_bidi_class_v1!(Baked);
    #[cfg(feature = "alloc")]
    impl_property_name_long_canonical_combining_class_v1!(Baked);
    impl_property_name_long_east_asian_width_v1!(Baked);
    impl_property_name_long_general_category_v1!(Baked);
    impl_property_name_long_grapheme_cluster_break_v1!(Baked);
    impl_property_name_long_hangul_syllable_type_v1!(Baked);
    impl_property_name_long_indic_syllabic_category_v1!(Baked);
    impl_property_name_long_joining_type_v1!(Baked);
    impl_property_name_long_line_break_v1!(Baked);
    impl_property_name_long_script_v1!(Baked);
    impl_property_name_long_sentence_break_v1!(Baked);
    impl_property_name_long_vertical_orientation_v1!(Baked);
    impl_property_name_long_word_break_v1!(Baked);
    impl_property_name_parse_bidi_class_v1!(Baked);
    impl_property_name_parse_canonical_combining_class_v1!(Baked);
    impl_property_name_parse_east_asian_width_v1!(Baked);
    impl_property_name_parse_general_category_mask_v1!(Baked);
    impl_property_name_parse_general_category_v1!(Baked);
    impl_property_name_parse_grapheme_cluster_break_v1!(Baked);
    impl_property_name_parse_hangul_syllable_type_v1!(Baked);
    impl_property_name_parse_indic_syllabic_category_v1!(Baked);
    impl_property_name_parse_joining_type_v1!(Baked);
    impl_property_name_parse_line_break_v1!(Baked);
    impl_property_name_parse_script_v1!(Baked);
    impl_property_name_parse_sentence_break_v1!(Baked);
    impl_property_name_parse_vertical_orientation_v1!(Baked);
    impl_property_name_parse_word_break_v1!(Baked);
    impl_property_name_short_bidi_class_v1!(Baked);
    #[cfg(feature = "alloc")]
    impl_property_name_short_canonical_combining_class_v1!(Baked);
    impl_property_name_short_east_asian_width_v1!(Baked);
    impl_property_name_short_general_category_v1!(Baked);
    impl_property_name_short_grapheme_cluster_break_v1!(Baked);
    impl_property_name_short_hangul_syllable_type_v1!(Baked);
    impl_property_name_short_indic_syllabic_category_v1!(Baked);
    impl_property_name_short_joining_type_v1!(Baked);
    impl_property_name_short_line_break_v1!(Baked);
    impl_property_name_short_script_v1!(Baked);
    impl_property_name_short_sentence_break_v1!(Baked);
    impl_property_name_short_vertical_orientation_v1!(Baked);
    impl_property_name_short_word_break_v1!(Baked);
    impl_property_script_with_extensions_v1!(Baked);
};

icu_provider::data_marker!(
    /// `PropertyBinaryAlnumV1`
    PropertyBinaryAlnumV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryAlphabeticV1`
    PropertyBinaryAlphabeticV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryAsciiHexDigitV1`
    PropertyBinaryAsciiHexDigitV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryBidiControlV1`
    PropertyBinaryBidiControlV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryBidiMirroredV1`
    PropertyBinaryBidiMirroredV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryBlankV1`
    PropertyBinaryBlankV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryCasedV1`
    PropertyBinaryCasedV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryCaseIgnorableV1`
    PropertyBinaryCaseIgnorableV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryCaseSensitiveV1`
    PropertyBinaryCaseSensitiveV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryChangesWhenCasefoldedV1`
    PropertyBinaryChangesWhenCasefoldedV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryChangesWhenCasemappedV1`
    PropertyBinaryChangesWhenCasemappedV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryChangesWhenLowercasedV1`
    PropertyBinaryChangesWhenLowercasedV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryChangesWhenNfkcCasefoldedV1`
    PropertyBinaryChangesWhenNfkcCasefoldedV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryChangesWhenTitlecasedV1`
    PropertyBinaryChangesWhenTitlecasedV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryChangesWhenUppercasedV1`
    PropertyBinaryChangesWhenUppercasedV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryDashV1`
    PropertyBinaryDashV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryDefaultIgnorableCodePointV1`
    PropertyBinaryDefaultIgnorableCodePointV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryDeprecatedV1`
    PropertyBinaryDeprecatedV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryDiacriticV1`
    PropertyBinaryDiacriticV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryEmojiComponentV1`
    PropertyBinaryEmojiComponentV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryEmojiModifierBaseV1`
    PropertyBinaryEmojiModifierBaseV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryEmojiModifierV1`
    PropertyBinaryEmojiModifierV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryEmojiPresentationV1`
    PropertyBinaryEmojiPresentationV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryEmojiV1`
    PropertyBinaryEmojiV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryExtendedPictographicV1`
    PropertyBinaryExtendedPictographicV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryExtenderV1`
    PropertyBinaryExtenderV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryFullCompositionExclusionV1`
    PropertyBinaryFullCompositionExclusionV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryGraphemeBaseV1`
    PropertyBinaryGraphemeBaseV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryGraphemeExtendV1`
    PropertyBinaryGraphemeExtendV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryGraphemeLinkV1`
    PropertyBinaryGraphemeLinkV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryGraphV1`
    PropertyBinaryGraphV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryHexDigitV1`
    PropertyBinaryHexDigitV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryHyphenV1`
    PropertyBinaryHyphenV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryIdCompatMathContinueV1`
    PropertyBinaryIdCompatMathContinueV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryIdCompatMathStartV1`
    PropertyBinaryIdCompatMathStartV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryIdContinueV1`
    PropertyBinaryIdContinueV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryIdeographicV1`
    PropertyBinaryIdeographicV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryIdsBinaryOperatorV1`
    PropertyBinaryIdsBinaryOperatorV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryIdStartV1`
    PropertyBinaryIdStartV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryIdsTrinaryOperatorV1`
    PropertyBinaryIdsTrinaryOperatorV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryIdsUnaryOperatorV1`
    PropertyBinaryIdsUnaryOperatorV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryJoinControlV1`
    PropertyBinaryJoinControlV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryLogicalOrderExceptionV1`
    PropertyBinaryLogicalOrderExceptionV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryLowercaseV1`
    PropertyBinaryLowercaseV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryMathV1`
    PropertyBinaryMathV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryModifierCombiningMarkV1`
    PropertyBinaryModifierCombiningMarkV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryNfcInertV1`
    PropertyBinaryNfcInertV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryNfdInertV1`
    PropertyBinaryNfdInertV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryNfkcInertV1`
    PropertyBinaryNfkcInertV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryNfkdInertV1`
    PropertyBinaryNfkdInertV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryNoncharacterCodePointV1`
    PropertyBinaryNoncharacterCodePointV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryPatternSyntaxV1`
    PropertyBinaryPatternSyntaxV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryPatternWhiteSpaceV1`
    PropertyBinaryPatternWhiteSpaceV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryPrependedConcatenationMarkV1`
    PropertyBinaryPrependedConcatenationMarkV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryPrintV1`
    PropertyBinaryPrintV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryQuotationMarkV1`
    PropertyBinaryQuotationMarkV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryRadicalV1`
    PropertyBinaryRadicalV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryRegionalIndicatorV1`
    PropertyBinaryRegionalIndicatorV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinarySegmentStarterV1`
    PropertyBinarySegmentStarterV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinarySentenceTerminalV1`
    PropertyBinarySentenceTerminalV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinarySoftDottedV1`
    PropertyBinarySoftDottedV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryTerminalPunctuationV1`
    PropertyBinaryTerminalPunctuationV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryUnifiedIdeographV1`
    PropertyBinaryUnifiedIdeographV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryUppercaseV1`
    PropertyBinaryUppercaseV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryVariationSelectorV1`
    PropertyBinaryVariationSelectorV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryWhiteSpaceV1`
    PropertyBinaryWhiteSpaceV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryXdigitV1`
    PropertyBinaryXdigitV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryXidContinueV1`
    PropertyBinaryXidContinueV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyBinaryXidStartV1`
    PropertyBinaryXidStartV1,
    PropertyCodePointSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// Data marker for the 'BidiClass' Unicode property
    PropertyEnumBidiClassV1,
    PropertyCodePointMap<'static, crate::props::BidiClass>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'CanonicalCombiningClass' Unicode property
    PropertyEnumCanonicalCombiningClassV1,
    PropertyCodePointMap<'static, crate::props::CanonicalCombiningClass>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'EastAsianWidth' Unicode property
    PropertyEnumEastAsianWidthV1,
    PropertyCodePointMap<'static, crate::props::EastAsianWidth>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'GeneralCategory' Unicode property
    PropertyEnumGeneralCategoryV1,
    PropertyCodePointMap<'static, crate::props::GeneralCategory>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'GraphemeClusterBreak' Unicode property
    PropertyEnumGraphemeClusterBreakV1,
    PropertyCodePointMap<'static, crate::props::GraphemeClusterBreak>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'HangulSyllableType' Unicode property
    PropertyEnumHangulSyllableTypeV1,
    PropertyCodePointMap<'static, crate::props::HangulSyllableType>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'IndicConjunctBreak' Unicode property
    PropertyEnumIndicConjunctBreakV1,
    PropertyCodePointMap<'static, crate::props::IndicConjunctBreak>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'IndicSyllabicCategory' Unicode property
    PropertyEnumIndicSyllabicCategoryV1,
    PropertyCodePointMap<'static, crate::props::IndicSyllabicCategory>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'JoiningType' Unicode property
    PropertyEnumJoiningTypeV1,
    PropertyCodePointMap<'static, crate::props::JoiningType>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'LineBreak' Unicode property
    PropertyEnumLineBreakV1,
    PropertyCodePointMap<'static, crate::props::LineBreak>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'Script' Unicode property
    PropertyEnumScriptV1,
    PropertyCodePointMap<'static, crate::props::Script>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'SentenceBreak' Unicode property
    PropertyEnumSentenceBreakV1,
    PropertyCodePointMap<'static, crate::props::SentenceBreak>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'Vertical_Orientation' Unicode property
    PropertyEnumVerticalOrientationV1,
    PropertyCodePointMap<'static, crate::props::VerticalOrientation>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'WordBreak' Unicode property
    PropertyEnumWordBreakV1,
    PropertyCodePointMap<'static, crate::props::WordBreak>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// Data marker for the 'BidiMirroringGlyph' Unicode property
    PropertyEnumBidiMirroringGlyphV1,
    PropertyCodePointMap<'static, crate::bidi::BidiMirroringGlyph>,
    is_singleton = true,
);
icu_provider::data_marker!(
    /// `PropertyBinaryBasicEmojiV1`
    PropertyBinaryBasicEmojiV1,
    PropertyUnicodeSet<'static>,
    is_singleton = true
);
icu_provider::data_marker!(
    /// `PropertyScriptWithExtensionsV1`
    PropertyScriptWithExtensionsV1,
    ScriptWithExtensionsProperty<'static>,
    is_singleton = true
);

/// All data keys in this module.
pub const MARKERS: &[DataMarkerInfo] = &[
    PropertyNameLongBidiClassV1::INFO,
    #[cfg(feature = "alloc")]
    PropertyNameLongCanonicalCombiningClassV1::INFO,
    PropertyNameLongEastAsianWidthV1::INFO,
    PropertyNameLongGeneralCategoryV1::INFO,
    PropertyNameLongGraphemeClusterBreakV1::INFO,
    PropertyNameLongHangulSyllableTypeV1::INFO,
    PropertyNameLongIndicSyllabicCategoryV1::INFO,
    PropertyNameLongJoiningTypeV1::INFO,
    PropertyNameLongLineBreakV1::INFO,
    PropertyNameLongScriptV1::INFO,
    PropertyNameLongSentenceBreakV1::INFO,
    PropertyNameLongVerticalOrientationV1::INFO,
    PropertyNameLongWordBreakV1::INFO,
    PropertyNameParseBidiClassV1::INFO,
    PropertyNameParseCanonicalCombiningClassV1::INFO,
    PropertyNameParseEastAsianWidthV1::INFO,
    PropertyNameParseGeneralCategoryMaskV1::INFO,
    PropertyNameParseGeneralCategoryV1::INFO,
    PropertyNameParseGraphemeClusterBreakV1::INFO,
    PropertyNameParseHangulSyllableTypeV1::INFO,
    PropertyNameParseIndicSyllabicCategoryV1::INFO,
    PropertyNameParseJoiningTypeV1::INFO,
    PropertyNameParseLineBreakV1::INFO,
    PropertyNameParseScriptV1::INFO,
    PropertyNameParseSentenceBreakV1::INFO,
    PropertyNameParseVerticalOrientationV1::INFO,
    PropertyNameParseWordBreakV1::INFO,
    PropertyNameShortBidiClassV1::INFO,
    #[cfg(feature = "alloc")]
    PropertyNameShortCanonicalCombiningClassV1::INFO,
    PropertyNameShortEastAsianWidthV1::INFO,
    PropertyNameShortGeneralCategoryV1::INFO,
    PropertyNameShortGraphemeClusterBreakV1::INFO,
    PropertyNameShortHangulSyllableTypeV1::INFO,
    PropertyNameShortIndicSyllabicCategoryV1::INFO,
    PropertyNameShortJoiningTypeV1::INFO,
    PropertyNameShortLineBreakV1::INFO,
    PropertyNameShortScriptV1::INFO,
    PropertyNameShortSentenceBreakV1::INFO,
    PropertyNameShortVerticalOrientationV1::INFO,
    PropertyNameShortWordBreakV1::INFO,
    PropertyBinaryAlnumV1::INFO,
    PropertyBinaryAlphabeticV1::INFO,
    PropertyBinaryAsciiHexDigitV1::INFO,
    PropertyBinaryBidiControlV1::INFO,
    PropertyBinaryBidiMirroredV1::INFO,
    PropertyBinaryBlankV1::INFO,
    PropertyBinaryCasedV1::INFO,
    PropertyBinaryCaseIgnorableV1::INFO,
    PropertyBinaryCaseSensitiveV1::INFO,
    PropertyBinaryChangesWhenCasefoldedV1::INFO,
    PropertyBinaryChangesWhenCasemappedV1::INFO,
    PropertyBinaryChangesWhenLowercasedV1::INFO,
    PropertyBinaryChangesWhenNfkcCasefoldedV1::INFO,
    PropertyBinaryChangesWhenTitlecasedV1::INFO,
    PropertyBinaryChangesWhenUppercasedV1::INFO,
    PropertyBinaryDashV1::INFO,
    PropertyBinaryDefaultIgnorableCodePointV1::INFO,
    PropertyBinaryDeprecatedV1::INFO,
    PropertyBinaryDiacriticV1::INFO,
    PropertyBinaryEmojiComponentV1::INFO,
    PropertyBinaryEmojiModifierBaseV1::INFO,
    PropertyBinaryEmojiModifierV1::INFO,
    PropertyBinaryEmojiPresentationV1::INFO,
    PropertyBinaryEmojiV1::INFO,
    PropertyBinaryExtendedPictographicV1::INFO,
    PropertyBinaryExtenderV1::INFO,
    PropertyBinaryFullCompositionExclusionV1::INFO,
    PropertyBinaryGraphemeBaseV1::INFO,
    PropertyBinaryGraphemeExtendV1::INFO,
    PropertyBinaryGraphemeLinkV1::INFO,
    PropertyBinaryGraphV1::INFO,
    PropertyBinaryHexDigitV1::INFO,
    PropertyBinaryHyphenV1::INFO,
    PropertyBinaryIdCompatMathContinueV1::INFO,
    PropertyBinaryIdCompatMathStartV1::INFO,
    PropertyBinaryIdContinueV1::INFO,
    PropertyBinaryIdeographicV1::INFO,
    PropertyBinaryIdsBinaryOperatorV1::INFO,
    PropertyBinaryIdStartV1::INFO,
    PropertyBinaryIdsTrinaryOperatorV1::INFO,
    PropertyBinaryIdsUnaryOperatorV1::INFO,
    PropertyBinaryJoinControlV1::INFO,
    PropertyBinaryLogicalOrderExceptionV1::INFO,
    PropertyBinaryLowercaseV1::INFO,
    PropertyBinaryMathV1::INFO,
    PropertyBinaryModifierCombiningMarkV1::INFO,
    PropertyBinaryNfcInertV1::INFO,
    PropertyBinaryNfdInertV1::INFO,
    PropertyBinaryNfkcInertV1::INFO,
    PropertyBinaryNfkdInertV1::INFO,
    PropertyBinaryNoncharacterCodePointV1::INFO,
    PropertyBinaryPatternSyntaxV1::INFO,
    PropertyBinaryPatternWhiteSpaceV1::INFO,
    PropertyBinaryPrependedConcatenationMarkV1::INFO,
    PropertyBinaryPrintV1::INFO,
    PropertyBinaryQuotationMarkV1::INFO,
    PropertyBinaryRadicalV1::INFO,
    PropertyBinaryRegionalIndicatorV1::INFO,
    PropertyBinarySegmentStarterV1::INFO,
    PropertyBinarySentenceTerminalV1::INFO,
    PropertyBinarySoftDottedV1::INFO,
    PropertyBinaryTerminalPunctuationV1::INFO,
    PropertyBinaryUnifiedIdeographV1::INFO,
    PropertyBinaryUppercaseV1::INFO,
    PropertyBinaryVariationSelectorV1::INFO,
    PropertyBinaryWhiteSpaceV1::INFO,
    PropertyBinaryXdigitV1::INFO,
    PropertyBinaryXidContinueV1::INFO,
    PropertyBinaryXidStartV1::INFO,
    PropertyEnumBidiClassV1::INFO,
    PropertyEnumCanonicalCombiningClassV1::INFO,
    PropertyEnumEastAsianWidthV1::INFO,
    PropertyEnumGeneralCategoryV1::INFO,
    PropertyEnumGraphemeClusterBreakV1::INFO,
    PropertyEnumHangulSyllableTypeV1::INFO,
    PropertyEnumIndicConjunctBreakV1::INFO,
    PropertyEnumIndicSyllabicCategoryV1::INFO,
    PropertyEnumJoiningTypeV1::INFO,
    PropertyEnumLineBreakV1::INFO,
    PropertyEnumScriptV1::INFO,
    PropertyEnumSentenceBreakV1::INFO,
    PropertyEnumVerticalOrientationV1::INFO,
    PropertyEnumWordBreakV1::INFO,
    PropertyEnumBidiMirroringGlyphV1::INFO,
    PropertyBinaryBasicEmojiV1::INFO,
    PropertyScriptWithExtensionsV1::INFO,
];

/// A set of characters which share a particular property value.
///
/// This data enum is extensible, more backends may be added in the future.
/// Old data can be used with newer code but not vice versa.
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[derive(Debug, Eq, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_properties::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[non_exhaustive]
pub enum PropertyCodePointSet<'data> {
    /// The set of characters, represented as an inversion list
    InversionList(#[cfg_attr(feature = "serde", serde(borrow))] CodePointInversionList<'data>),
    // new variants should go BELOW existing ones
    // Serde serializes based on variant name and index in the enum
    // https://docs.rs/serde/latest/serde/trait.Serializer.html#tymethod.serialize_unit_variant
}

icu_provider::data_struct!(
    PropertyCodePointSet<'_>,
    #[cfg(feature = "datagen")]
);

// See CodePointSetData for documentation of these functions
impl<'data> PropertyCodePointSet<'data> {
    #[inline]
    pub(crate) fn contains(&self, ch: char) -> bool {
        match *self {
            Self::InversionList(ref l) => l.contains(ch),
        }
    }

    #[inline]
    pub(crate) fn contains32(&self, ch: u32) -> bool {
        match *self {
            Self::InversionList(ref l) => l.contains32(ch),
        }
    }

    #[inline]
    pub(crate) fn iter_ranges(&self) -> impl Iterator<Item = RangeInclusive<u32>> + '_ {
        match *self {
            Self::InversionList(ref l) => l.iter_ranges(),
        }
    }

    #[inline]
    pub(crate) fn iter_ranges_complemented(
        &self,
    ) -> impl Iterator<Item = RangeInclusive<u32>> + '_ {
        match *self {
            Self::InversionList(ref l) => l.iter_ranges_complemented(),
        }
    }

    #[inline]
    pub(crate) fn from_code_point_inversion_list(l: CodePointInversionList<'static>) -> Self {
        Self::InversionList(l)
    }

    #[inline]
    pub(crate) fn as_code_point_inversion_list(
        &'_ self,
    ) -> Option<&'_ CodePointInversionList<'data>> {
        match *self {
            Self::InversionList(ref l) => Some(l),
            // any other backing data structure that cannot return a CPInvList in O(1) time should return None
        }
    }

    #[inline]
    pub(crate) fn to_code_point_inversion_list(&self) -> CodePointInversionList<'_> {
        match *self {
            Self::InversionList(ref t) => ZeroFrom::zero_from(t),
        }
    }
}

/// A map efficiently storing data about individual characters.
///
/// This data enum is extensible, more backends may be added in the future.
/// Old data can be used with newer code but not vice versa.
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[derive(Clone, Debug, Eq, PartialEq, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_properties::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[non_exhaustive]
pub enum PropertyCodePointMap<'data, T: TrieValue> {
    /// A codepoint trie storing the data
    CodePointTrie(#[cfg_attr(feature = "serde", serde(borrow))] CodePointTrie<'data, T>),
    // new variants should go BELOW existing ones
    // Serde serializes based on variant name and index in the enum
    // https://docs.rs/serde/latest/serde/trait.Serializer.html#tymethod.serialize_unit_variant
}

icu_provider::data_struct!(
    <T: TrieValue> PropertyCodePointMap<'_, T>,
    #[cfg(feature = "datagen")]
);

// See CodePointMapData for documentation of these functions
impl<'data, T: TrieValue> PropertyCodePointMap<'data, T> {
    #[inline]
    pub(crate) fn get32(&self, ch: u32) -> T {
        match *self {
            Self::CodePointTrie(ref t) => t.get32(ch),
        }
    }

    #[inline]
    pub(crate) fn get(&self, c: char) -> T {
        match *self {
            Self::CodePointTrie(ref t) => t.get(c),
        }
    }

    #[inline]
    #[cfg(feature = "alloc")]
    pub(crate) fn try_into_converted<P>(
        self,
    ) -> Result<PropertyCodePointMap<'data, P>, zerovec::ule::UleError>
    where
        P: TrieValue,
    {
        match self {
            Self::CodePointTrie(t) => t
                .try_into_converted()
                .map(PropertyCodePointMap::CodePointTrie),
        }
    }

    #[inline]
    #[cfg(feature = "alloc")]
    pub(crate) fn get_set_for_value(&self, value: T) -> CodePointInversionList<'static> {
        match *self {
            Self::CodePointTrie(ref t) => t.get_set_for_value(value),
        }
    }

    #[inline]
    pub(crate) fn iter_ranges(&self) -> impl Iterator<Item = CodePointMapRange<T>> + '_ {
        match *self {
            Self::CodePointTrie(ref t) => t.iter_ranges(),
        }
    }
    #[inline]
    pub(crate) fn iter_ranges_mapped<'a, U: Eq + 'a>(
        &'a self,
        map: impl FnMut(T) -> U + Copy + 'a,
    ) -> impl Iterator<Item = CodePointMapRange<U>> + 'a {
        match *self {
            Self::CodePointTrie(ref t) => t.iter_ranges_mapped(map),
        }
    }

    #[inline]
    pub(crate) fn from_code_point_trie(trie: CodePointTrie<'static, T>) -> Self {
        Self::CodePointTrie(trie)
    }

    #[inline]
    pub(crate) fn as_code_point_trie(&self) -> Option<&CodePointTrie<'data, T>> {
        match *self {
            Self::CodePointTrie(ref t) => Some(t),
            // any other backing data structure that cannot return a CPT in O(1) time should return None
        }
    }

    #[inline]
    pub(crate) fn to_code_point_trie(&self) -> CodePointTrie<'_, T> {
        match *self {
            Self::CodePointTrie(ref t) => ZeroFrom::zero_from(t),
        }
    }
}

/// A set of characters and strings which share a particular property value.
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[derive(Debug, Eq, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_properties::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[non_exhaustive]
pub enum PropertyUnicodeSet<'data> {
    /// A set representing characters in an inversion list, and the strings in a list.
    CPInversionListStrList(
        #[cfg_attr(feature = "serde", serde(borrow))] CodePointInversionListAndStringList<'data>,
    ),
    // new variants should go BELOW existing ones
    // Serde serializes based on variant name and index in the enum
    // https://docs.rs/serde/latest/serde/trait.Serializer.html#tymethod.serialize_unit_variant
}

icu_provider::data_struct!(
    PropertyUnicodeSet<'_>,
    #[cfg(feature = "datagen")]
);

impl<'data> PropertyUnicodeSet<'data> {
    #[inline]
    pub(crate) fn contains_str(&self, s: &str) -> bool {
        match *self {
            Self::CPInversionListStrList(ref l) => l.contains_str(s),
        }
    }

    #[inline]
    pub(crate) fn contains32(&self, cp: u32) -> bool {
        match *self {
            Self::CPInversionListStrList(ref l) => l.contains32(cp),
        }
    }

    #[inline]
    pub(crate) fn contains(&self, ch: char) -> bool {
        match *self {
            Self::CPInversionListStrList(ref l) => l.contains(ch),
        }
    }

    #[inline]
    pub(crate) fn from_code_point_inversion_list_string_list(
        l: CodePointInversionListAndStringList<'static>,
    ) -> Self {
        Self::CPInversionListStrList(l)
    }

    #[inline]
    pub(crate) fn as_code_point_inversion_list_string_list(
        &'_ self,
    ) -> Option<&'_ CodePointInversionListAndStringList<'data>> {
        match *self {
            Self::CPInversionListStrList(ref l) => Some(l),
            // any other backing data structure that cannot return a CPInversionListStrList in O(1) time should return None
        }
    }

    #[inline]
    pub(crate) fn to_code_point_inversion_list_string_list(
        &self,
    ) -> CodePointInversionListAndStringList<'_> {
        match *self {
            Self::CPInversionListStrList(ref t) => ZeroFrom::zero_from(t),
        }
    }
}

/// A struct that efficiently stores `Script` and `Script_Extensions` property data.
///
/// <div class="stab unstable">
/// 🚧 This code is considered unstable; it may change at any time, in breaking or non-breaking ways,
/// including in SemVer minor releases. While the serde representation of data structs is guaranteed
/// to be stable, their Rust representation might not be. Use with caution.
/// </div>
#[derive(Debug, Eq, PartialEq, Clone, yoke::Yokeable, zerofrom::ZeroFrom)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_properties::provider))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct ScriptWithExtensionsProperty<'data> {
    /// Note: The `ScriptWithExt` values in this array will assume a 12-bit layout. The 2
    /// higher order bits 11..10 will indicate how to deduce the Script value and
    /// Script_Extensions value, nearly matching the representation
    /// [in ICU](https://github.com/unicode-org/icu/blob/main/icu4c/source/common/uprops.h):
    ///
    /// | High order 2 bits value | Script                                                 | Script_Extensions                                              |
    /// |-------------------------|--------------------------------------------------------|----------------------------------------------------------------|
    /// | 3                       | First value in sub-array, index given by lower 10 bits | Sub-array excluding first value, index given by lower 10 bits  |
    /// | 2                       | Script=Inherited                                       | Entire sub-array, index given by lower 10 bits                 |
    /// | 1                       | Script=Common                                          | Entire sub-array, index given by lower 10 bits                 |
    /// | 0                       | Value in lower 10 bits                                 | `[ Script value ]` single-element array                        |
    ///
    /// When the lower 10 bits of the value are used as an index, that index is
    /// used for the outer-level vector of the nested `extensions` structure.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub trie: CodePointTrie<'data, ScriptWithExt>,

    /// This companion structure stores Script_Extensions values, which are
    /// themselves arrays / vectors. This structure only stores the values for
    /// cases in which `scx(cp) != [ sc(cp) ]`. Each sub-vector is distinct. The
    /// sub-vector represents the Script_Extensions array value for a code point,
    /// and may also indicate Script value, as described for the `trie` field.
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub extensions: VarZeroVec<'data, ZeroSlice<Script>>,
}

icu_provider::data_struct!(
    ScriptWithExtensionsProperty<'_>,
    #[cfg(feature = "datagen")]
);
