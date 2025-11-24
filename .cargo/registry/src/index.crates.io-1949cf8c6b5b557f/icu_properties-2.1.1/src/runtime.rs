// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! 🚧 \[Experimental\] This module is experimental and currently crate-private. Let us know if you
//! have a use case for this!
//!
//! This module contains utilities for working with properties where the specific property in use
//! is not known at compile time.
//!
//! For regex engines, [`crate::sets::load_for_ecma262_unstable()`] is a convenient API for working
//! with properties at runtime tailored for the use case of ECMA262-compatible regex engines.

use crate::provider::*;
use crate::CodePointSetData;
#[cfg(doc)]
use crate::{
    props::{GeneralCategory, GeneralCategoryGroup, Script},
    script, CodePointMapData, PropertyParser,
};
use icu_provider::prelude::*;

/// This type can represent any binary Unicode property.
///
/// This is intended to be used in situations where the exact unicode property needed is
/// only known at runtime, for example in regex engines.
///
/// The values are intended to be identical to ICU4C's UProperty enum
#[non_exhaustive]
#[allow(missing_docs)]
#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum BinaryProperty {
    Alnum = 44,
    Alphabetic = 0,
    AsciiHexDigit = 1,
    BidiControl = 2,
    BidiMirrored = 3,
    Blank = 45,
    Cased = 49,
    CaseIgnorable = 50,
    CaseSensitive = 34,
    ChangesWhenCasefolded = 54,
    ChangesWhenCasemapped = 55,
    ChangesWhenLowercased = 51,
    ChangesWhenNfkcCasefolded = 56,
    ChangesWhenTitlecased = 53,
    ChangesWhenUppercased = 52,
    Dash = 4,
    DefaultIgnorableCodePoint = 5,
    Deprecated = 6,
    Diacritic = 7,
    Emoji = 57,
    EmojiComponent = 61,
    EmojiModifier = 59,
    EmojiModifierBase = 60,
    EmojiPresentation = 58,
    ExtendedPictographic = 64,
    Extender = 8,
    FullCompositionExclusion = 9,
    Graph = 46,
    GraphemeBase = 10,
    GraphemeExtend = 11,
    GraphemeLink = 12,
    HexDigit = 13,
    Hyphen = 14,
    IdCompatMathContinue = 65,
    IdCompatMathStart = 66,
    IdContinue = 15,
    Ideographic = 17,
    IdsBinaryOperator = 18,
    IdStart = 16,
    IdsTrinaryOperator = 19,
    IdsUnaryOperator = 67,
    JoinControl = 20,
    LogicalOrderException = 21,
    Lowercase = 22,
    Math = 23,
    ModifierCombiningMark = 68,
    NfcInert = 39,
    NfdInert = 37,
    NfkcInert = 40,
    NfkdInert = 38,
    NoncharacterCodePoint = 24,
    PatternSyntax = 42,
    PatternWhiteSpace = 43,
    PrependedConcatenationMark = 63,
    Print = 47,
    QuotationMark = 25,
    Radical = 26,
    RegionalIndicator = 62,
    SegmentStarter = 41,
    SentenceTerminal = 35,
    SoftDotted = 27,
    TerminalPunctuation = 28,
    UnifiedIdeograph = 29,
    Uppercase = 30,
    VariationSelector = 36,
    WhiteSpace = 31,
    Xdigit = 48,
    XidContinue = 32,
    XidStart = 33,
}

/// This type can represent any binary property over strings.
///
/// This is intended to be used in situations where the exact unicode property needed is
/// only known at runtime, for example in regex engines.
///
/// The values are intended to be identical to ICU4C's UProperty enum
#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[allow(dead_code)]
#[allow(missing_docs)]
enum StringBinaryProperty {
    BasicEmoji = 65,
    EmojiKeycapSequence = 66,
    RgiEmoji = 71,
    RgiEmojiFlagSequence = 68,
    RgiEmojiModifierSequence = 67,
    RgiEmojiTagSequence = 69,
    RgiEmojiZWJSequence = 70,
}

/// This type can represent any enumerated Unicode property.
///
/// This is intended to be used in situations where the exact unicode property needed is
/// only known at runtime, for example in regex engines.
///
/// The values are intended to be identical to ICU4C's UProperty enum
#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[allow(dead_code)]
#[allow(missing_docs)]
enum EnumeratedProperty {
    BidiClass = 0x1000,
    BidiPairedBracketType = 0x1015,
    Block = 0x1001,
    CombiningClass = 0x1002,
    DecompositionType = 0x1003,
    EastAsianWidth = 0x1004,
    GeneralCategory = 0x1005,
    GraphemeClusterBreak = 0x1012,
    HangulSyllableType = 0x100B,
    IndicConjunctBreak = 0x101A,
    IndicPositionalCategory = 0x1016,
    IndicSyllabicCategory = 0x1017,
    JoiningGroup = 0x1006,
    JoiningType = 0x1007,
    LeadCanonicalCombiningClass = 0x1010,
    LineBreak = 0x1008,
    NFCQuickCheck = 0x100E,
    NFDQuickCheck = 0x100C,
    NFKCQuickCheck = 0x100F,
    NFKDQuickCheck = 0x100D,
    NumericType = 0x1009,
    Script = 0x100A,
    SentenceBreak = 0x1013,
    TrailCanonicalCombiningClass = 0x1011,
    VerticalOrientation = 0x1018,
    WordBreak = 0x1014,
}

/// This type can represent any Unicode mask property.
///
/// This is intended to be used in situations where the exact unicode property needed is
/// only known at runtime, for example in regex engines.
///
/// The values are intended to be identical to ICU4C's UProperty enum
#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[allow(dead_code)]
#[allow(missing_docs)]
enum MaskProperty {
    GeneralCategoryMask = 0x2000,
}

/// This type can represent any numeric Unicode property.
///
/// This is intended to be used in situations where the exact unicode property needed is
/// only known at runtime, for example in regex engines.
///
/// The values are intended to be identical to ICU4C's UProperty enum
#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[allow(dead_code)]
#[allow(missing_docs)]
enum NumericProperty {
    NumericValue = 0x3000,
}

/// This type can represent any Unicode string property.
///
/// This is intended to be used in situations where the exact unicode property needed is
/// only known at runtime, for example in regex engines.
///
/// The values are intended to be identical to ICU4C's UProperty enum
#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[allow(dead_code)]
#[allow(missing_docs)]
enum StringProperty {
    Age = 0x4000,
    BidiMirroringGlyph = 0x4001,
    BidiPairedBracket = 0x400D,
    CaseFolding = 0x4002,
    ISOComment = 0x4003,
    LowercaseMapping = 0x4004,
    Name = 0x4005,
    SimpleCaseFolding = 0x4006,
    SimpleLowercaseMapping = 0x4007,
    SimpleTitlecaseMapping = 0x4008,
    SimpleUppercaseMapping = 0x4009,
    TitlecaseMapping = 0x400A,
    Unicode1Name = 0x400B,
    UppercaseMapping = 0x400C,
}

#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[allow(dead_code)]
#[allow(missing_docs)]
enum MiscProperty {
    ScriptExtensions = 0x7000,
}

impl CodePointSetData {
    /// Returns a type capable of looking up values for a property specified as a string, as long as it is a
    /// [binary property listed in ECMA-262][ecma], using strict matching on the names in the spec.
    ///
    /// This handles every property required by ECMA-262 `/u` regular expressions, except for:
    ///
    /// - `Script` and `General_Category`: handle these directly using property values parsed via
    ///   [`PropertyParser<GeneralCategory>`] and [`PropertyParser<Script>`]
    ///   if necessary.
    /// - `Script_Extensions`: handle this directly using APIs from [`crate::script::ScriptWithExtensions`]
    /// - `General_Category` mask values: Handle this alongside `General_Category` using [`GeneralCategoryGroup`],
    ///   using property values parsed via [`PropertyParser<GeneralCategory>`] if necessary
    /// - `Assigned`, `All`, and `ASCII` pseudoproperties: Handle these using their equivalent sets:
    ///    - `Any` can be expressed as the range `[\u{0}-\u{10FFFF}]`
    ///    - `Assigned` can be expressed as the inverse of the set `gc=Cn` (i.e., `\P{gc=Cn}`).
    ///    - `ASCII` can be expressed as the range `[\u{0}-\u{7F}]`
    /// - `General_Category` property values can themselves be treated like properties using a shorthand in ECMA262,
    ///   simply create the corresponding `GeneralCategory` set.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    ///
    /// ```
    /// use icu::properties::CodePointSetData;
    ///
    /// let emoji = CodePointSetData::new_for_ecma262(b"Emoji")
    ///     .expect("is an ECMA-262 property");
    ///
    /// assert!(emoji.contains('🔥')); // U+1F525 FIRE
    /// assert!(!emoji.contains('V'));
    /// ```
    ///
    /// [ecma]: https://tc39.es/ecma262/#table-binary-unicode-properties
    #[cfg(feature = "compiled_data")]
    pub fn new_for_ecma262(prop: &[u8]) -> Option<crate::CodePointSetDataBorrowed<'static>> {
        use crate::props::*;
        Some(match prop {
            AsciiHexDigit::NAME | AsciiHexDigit::SHORT_NAME => Self::new::<AsciiHexDigit>(),
            Alphabetic::NAME | Alphabetic::SHORT_NAME => Self::new::<Alphabetic>(),
            BidiControl::NAME | BidiControl::SHORT_NAME => Self::new::<BidiControl>(),
            BidiMirrored::NAME | BidiMirrored::SHORT_NAME => Self::new::<BidiMirrored>(),
            CaseIgnorable::NAME | CaseIgnorable::SHORT_NAME => Self::new::<CaseIgnorable>(),
            #[allow(unreachable_patterns)] // no short name
            Cased::NAME | Cased::SHORT_NAME => Self::new::<Cased>(),
            ChangesWhenCasefolded::NAME | ChangesWhenCasefolded::SHORT_NAME => {
                Self::new::<ChangesWhenCasefolded>()
            }
            ChangesWhenCasemapped::NAME | ChangesWhenCasemapped::SHORT_NAME => {
                Self::new::<ChangesWhenCasemapped>()
            }
            ChangesWhenLowercased::NAME | ChangesWhenLowercased::SHORT_NAME => {
                Self::new::<ChangesWhenLowercased>()
            }
            ChangesWhenNfkcCasefolded::NAME | ChangesWhenNfkcCasefolded::SHORT_NAME => {
                Self::new::<ChangesWhenNfkcCasefolded>()
            }
            ChangesWhenTitlecased::NAME | ChangesWhenTitlecased::SHORT_NAME => {
                Self::new::<ChangesWhenTitlecased>()
            }
            ChangesWhenUppercased::NAME | ChangesWhenUppercased::SHORT_NAME => {
                Self::new::<ChangesWhenUppercased>()
            }
            #[allow(unreachable_patterns)] // no short name
            Dash::NAME | Dash::SHORT_NAME => Self::new::<Dash>(),
            DefaultIgnorableCodePoint::NAME | DefaultIgnorableCodePoint::SHORT_NAME => {
                Self::new::<DefaultIgnorableCodePoint>()
            }
            Deprecated::NAME | Deprecated::SHORT_NAME => Self::new::<Deprecated>(),
            Diacritic::NAME | Diacritic::SHORT_NAME => Self::new::<Diacritic>(),
            #[allow(unreachable_patterns)] // no short name
            Emoji::NAME | Emoji::SHORT_NAME => Self::new::<Emoji>(),
            EmojiComponent::NAME | EmojiComponent::SHORT_NAME => Self::new::<EmojiComponent>(),
            EmojiModifier::NAME | EmojiModifier::SHORT_NAME => Self::new::<EmojiModifier>(),
            EmojiModifierBase::NAME | EmojiModifierBase::SHORT_NAME => {
                Self::new::<EmojiModifierBase>()
            }
            EmojiPresentation::NAME | EmojiPresentation::SHORT_NAME => {
                Self::new::<EmojiPresentation>()
            }
            ExtendedPictographic::NAME | ExtendedPictographic::SHORT_NAME => {
                Self::new::<ExtendedPictographic>()
            }
            Extender::NAME | Extender::SHORT_NAME => Self::new::<Extender>(),
            GraphemeBase::NAME | GraphemeBase::SHORT_NAME => Self::new::<GraphemeBase>(),
            GraphemeExtend::NAME | GraphemeExtend::SHORT_NAME => Self::new::<GraphemeExtend>(),
            HexDigit::NAME | HexDigit::SHORT_NAME => Self::new::<HexDigit>(),
            IdsBinaryOperator::NAME | IdsBinaryOperator::SHORT_NAME => {
                Self::new::<IdsBinaryOperator>()
            }
            IdsTrinaryOperator::NAME | IdsTrinaryOperator::SHORT_NAME => {
                Self::new::<IdsTrinaryOperator>()
            }
            IdContinue::NAME | IdContinue::SHORT_NAME => Self::new::<IdContinue>(),
            IdStart::NAME | IdStart::SHORT_NAME => Self::new::<IdStart>(),
            Ideographic::NAME | Ideographic::SHORT_NAME => Self::new::<Ideographic>(),
            JoinControl::NAME | JoinControl::SHORT_NAME => Self::new::<JoinControl>(),
            LogicalOrderException::NAME | LogicalOrderException::SHORT_NAME => {
                Self::new::<LogicalOrderException>()
            }
            Lowercase::NAME | Lowercase::SHORT_NAME => Self::new::<Lowercase>(),
            #[allow(unreachable_patterns)] // no short name
            Math::NAME | Math::SHORT_NAME => Self::new::<Math>(),
            NoncharacterCodePoint::NAME | NoncharacterCodePoint::SHORT_NAME => {
                Self::new::<NoncharacterCodePoint>()
            }
            PatternSyntax::NAME | PatternSyntax::SHORT_NAME => Self::new::<PatternSyntax>(),
            PatternWhiteSpace::NAME | PatternWhiteSpace::SHORT_NAME => {
                Self::new::<PatternWhiteSpace>()
            }
            QuotationMark::NAME | QuotationMark::SHORT_NAME => Self::new::<QuotationMark>(),
            #[allow(unreachable_patterns)] // no short name
            Radical::NAME | Radical::SHORT_NAME => Self::new::<Radical>(),
            RegionalIndicator::NAME | RegionalIndicator::SHORT_NAME => {
                Self::new::<RegionalIndicator>()
            }
            SentenceTerminal::NAME | SentenceTerminal::SHORT_NAME => {
                Self::new::<SentenceTerminal>()
            }
            SoftDotted::NAME | SoftDotted::SHORT_NAME => Self::new::<SoftDotted>(),
            TerminalPunctuation::NAME | TerminalPunctuation::SHORT_NAME => {
                Self::new::<TerminalPunctuation>()
            }
            UnifiedIdeograph::NAME | UnifiedIdeograph::SHORT_NAME => {
                Self::new::<UnifiedIdeograph>()
            }
            Uppercase::NAME | Uppercase::SHORT_NAME => Self::new::<Uppercase>(),
            VariationSelector::NAME | VariationSelector::SHORT_NAME => {
                Self::new::<VariationSelector>()
            }
            WhiteSpace::NAME | WhiteSpace::SHORT_NAME => Self::new::<WhiteSpace>(),
            XidContinue::NAME | XidContinue::SHORT_NAME => Self::new::<XidContinue>(),
            XidStart::NAME | XidStart::SHORT_NAME => Self::new::<XidStart>(),
            // Not an ECMA-262 property
            _ => return None,
        })
    }

    icu_provider::gen_buffer_data_constructors!(
        (prop: &[u8]) -> result: Option<Result<Self, DataError>>,
        functions: [
            new_for_ecma262: skip,
            try_new_for_ecma262_with_buffer_provider,
            try_new_for_ecma262_unstable,
            Self,
        ]
    );

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new_for_ecma262)]
    pub fn try_new_for_ecma262_unstable<P>(
        provider: &P,
        prop: &[u8],
    ) -> Option<Result<Self, DataError>>
    where
        P: ?Sized
            + DataProvider<PropertyBinaryAsciiHexDigitV1>
            + DataProvider<PropertyBinaryAlphabeticV1>
            + DataProvider<PropertyBinaryBidiControlV1>
            + DataProvider<PropertyBinaryBidiMirroredV1>
            + DataProvider<PropertyBinaryCaseIgnorableV1>
            + DataProvider<PropertyBinaryCasedV1>
            + DataProvider<PropertyBinaryChangesWhenCasefoldedV1>
            + DataProvider<PropertyBinaryChangesWhenCasemappedV1>
            + DataProvider<PropertyBinaryChangesWhenLowercasedV1>
            + DataProvider<PropertyBinaryChangesWhenNfkcCasefoldedV1>
            + DataProvider<PropertyBinaryChangesWhenTitlecasedV1>
            + DataProvider<PropertyBinaryChangesWhenUppercasedV1>
            + DataProvider<PropertyBinaryDashV1>
            + DataProvider<PropertyBinaryDefaultIgnorableCodePointV1>
            + DataProvider<PropertyBinaryDeprecatedV1>
            + DataProvider<PropertyBinaryDiacriticV1>
            + DataProvider<PropertyBinaryEmojiV1>
            + DataProvider<PropertyBinaryEmojiComponentV1>
            + DataProvider<PropertyBinaryEmojiModifierV1>
            + DataProvider<PropertyBinaryEmojiModifierBaseV1>
            + DataProvider<PropertyBinaryEmojiPresentationV1>
            + DataProvider<PropertyBinaryExtendedPictographicV1>
            + DataProvider<PropertyBinaryExtenderV1>
            + DataProvider<PropertyBinaryGraphemeBaseV1>
            + DataProvider<PropertyBinaryGraphemeExtendV1>
            + DataProvider<PropertyBinaryHexDigitV1>
            + DataProvider<PropertyBinaryIdsBinaryOperatorV1>
            + DataProvider<PropertyBinaryIdsTrinaryOperatorV1>
            + DataProvider<PropertyBinaryIdContinueV1>
            + DataProvider<PropertyBinaryIdStartV1>
            + DataProvider<PropertyBinaryIdeographicV1>
            + DataProvider<PropertyBinaryJoinControlV1>
            + DataProvider<PropertyBinaryLogicalOrderExceptionV1>
            + DataProvider<PropertyBinaryLowercaseV1>
            + DataProvider<PropertyBinaryMathV1>
            + DataProvider<PropertyBinaryNoncharacterCodePointV1>
            + DataProvider<PropertyBinaryPatternSyntaxV1>
            + DataProvider<PropertyBinaryPatternWhiteSpaceV1>
            + DataProvider<PropertyBinaryQuotationMarkV1>
            + DataProvider<PropertyBinaryRadicalV1>
            + DataProvider<PropertyBinaryRegionalIndicatorV1>
            + DataProvider<PropertyBinarySentenceTerminalV1>
            + DataProvider<PropertyBinarySoftDottedV1>
            + DataProvider<PropertyBinaryTerminalPunctuationV1>
            + DataProvider<PropertyBinaryUnifiedIdeographV1>
            + DataProvider<PropertyBinaryUppercaseV1>
            + DataProvider<PropertyBinaryVariationSelectorV1>
            + DataProvider<PropertyBinaryWhiteSpaceV1>
            + DataProvider<PropertyBinaryXidContinueV1>
            + DataProvider<PropertyBinaryXidStartV1>,
    {
        use crate::props::*;
        Some(match prop {
            AsciiHexDigit::NAME | AsciiHexDigit::SHORT_NAME => {
                Self::try_new_unstable::<AsciiHexDigit>(provider)
            }
            Alphabetic::NAME | Alphabetic::SHORT_NAME => {
                Self::try_new_unstable::<Alphabetic>(provider)
            }
            BidiControl::NAME | BidiControl::SHORT_NAME => {
                Self::try_new_unstable::<BidiControl>(provider)
            }
            BidiMirrored::NAME | BidiMirrored::SHORT_NAME => {
                Self::try_new_unstable::<BidiMirrored>(provider)
            }
            CaseIgnorable::NAME | CaseIgnorable::SHORT_NAME => {
                Self::try_new_unstable::<CaseIgnorable>(provider)
            }
            #[allow(unreachable_patterns)] // no short name
            Cased::NAME | Cased::SHORT_NAME => Self::try_new_unstable::<Cased>(provider),
            ChangesWhenCasefolded::NAME | ChangesWhenCasefolded::SHORT_NAME => {
                Self::try_new_unstable::<ChangesWhenCasefolded>(provider)
            }
            ChangesWhenCasemapped::NAME | ChangesWhenCasemapped::SHORT_NAME => {
                Self::try_new_unstable::<ChangesWhenCasemapped>(provider)
            }
            ChangesWhenLowercased::NAME | ChangesWhenLowercased::SHORT_NAME => {
                Self::try_new_unstable::<ChangesWhenLowercased>(provider)
            }
            ChangesWhenNfkcCasefolded::NAME | ChangesWhenNfkcCasefolded::SHORT_NAME => {
                Self::try_new_unstable::<ChangesWhenNfkcCasefolded>(provider)
            }
            ChangesWhenTitlecased::NAME | ChangesWhenTitlecased::SHORT_NAME => {
                Self::try_new_unstable::<ChangesWhenTitlecased>(provider)
            }
            ChangesWhenUppercased::NAME | ChangesWhenUppercased::SHORT_NAME => {
                Self::try_new_unstable::<ChangesWhenUppercased>(provider)
            }
            #[allow(unreachable_patterns)] // no short name
            Dash::NAME | Dash::SHORT_NAME => Self::try_new_unstable::<Dash>(provider),
            DefaultIgnorableCodePoint::NAME | DefaultIgnorableCodePoint::SHORT_NAME => {
                Self::try_new_unstable::<DefaultIgnorableCodePoint>(provider)
            }
            Deprecated::NAME | Deprecated::SHORT_NAME => {
                Self::try_new_unstable::<Deprecated>(provider)
            }
            Diacritic::NAME | Diacritic::SHORT_NAME => {
                Self::try_new_unstable::<Diacritic>(provider)
            }
            #[allow(unreachable_patterns)] // no short name
            Emoji::NAME | Emoji::SHORT_NAME => Self::try_new_unstable::<Emoji>(provider),
            EmojiComponent::NAME | EmojiComponent::SHORT_NAME => {
                Self::try_new_unstable::<EmojiComponent>(provider)
            }
            EmojiModifier::NAME | EmojiModifier::SHORT_NAME => {
                Self::try_new_unstable::<EmojiModifier>(provider)
            }
            EmojiModifierBase::NAME | EmojiModifierBase::SHORT_NAME => {
                Self::try_new_unstable::<EmojiModifierBase>(provider)
            }
            EmojiPresentation::NAME | EmojiPresentation::SHORT_NAME => {
                Self::try_new_unstable::<EmojiPresentation>(provider)
            }
            ExtendedPictographic::NAME | ExtendedPictographic::SHORT_NAME => {
                Self::try_new_unstable::<ExtendedPictographic>(provider)
            }
            Extender::NAME | Extender::SHORT_NAME => Self::try_new_unstable::<Extender>(provider),
            GraphemeBase::NAME | GraphemeBase::SHORT_NAME => {
                Self::try_new_unstable::<GraphemeBase>(provider)
            }
            GraphemeExtend::NAME | GraphemeExtend::SHORT_NAME => {
                Self::try_new_unstable::<GraphemeExtend>(provider)
            }
            HexDigit::NAME | HexDigit::SHORT_NAME => Self::try_new_unstable::<HexDigit>(provider),
            IdsBinaryOperator::NAME | IdsBinaryOperator::SHORT_NAME => {
                Self::try_new_unstable::<IdsBinaryOperator>(provider)
            }
            IdsTrinaryOperator::NAME | IdsTrinaryOperator::SHORT_NAME => {
                Self::try_new_unstable::<IdsTrinaryOperator>(provider)
            }
            IdContinue::NAME | IdContinue::SHORT_NAME => {
                Self::try_new_unstable::<IdContinue>(provider)
            }
            IdStart::NAME | IdStart::SHORT_NAME => Self::try_new_unstable::<IdStart>(provider),
            Ideographic::NAME | Ideographic::SHORT_NAME => {
                Self::try_new_unstable::<Ideographic>(provider)
            }
            JoinControl::NAME | JoinControl::SHORT_NAME => {
                Self::try_new_unstable::<JoinControl>(provider)
            }
            LogicalOrderException::NAME | LogicalOrderException::SHORT_NAME => {
                Self::try_new_unstable::<LogicalOrderException>(provider)
            }
            Lowercase::NAME | Lowercase::SHORT_NAME => {
                Self::try_new_unstable::<Lowercase>(provider)
            }
            #[allow(unreachable_patterns)] // no short name
            Math::NAME | Math::SHORT_NAME => Self::try_new_unstable::<Math>(provider),
            NoncharacterCodePoint::NAME | NoncharacterCodePoint::SHORT_NAME => {
                Self::try_new_unstable::<NoncharacterCodePoint>(provider)
            }
            PatternSyntax::NAME | PatternSyntax::SHORT_NAME => {
                Self::try_new_unstable::<PatternSyntax>(provider)
            }
            PatternWhiteSpace::NAME | PatternWhiteSpace::SHORT_NAME => {
                Self::try_new_unstable::<PatternWhiteSpace>(provider)
            }
            QuotationMark::NAME | QuotationMark::SHORT_NAME => {
                Self::try_new_unstable::<QuotationMark>(provider)
            }
            #[allow(unreachable_patterns)] // no short name
            Radical::NAME | Radical::SHORT_NAME => Self::try_new_unstable::<Radical>(provider),
            RegionalIndicator::NAME | RegionalIndicator::SHORT_NAME => {
                Self::try_new_unstable::<RegionalIndicator>(provider)
            }
            SentenceTerminal::NAME | SentenceTerminal::SHORT_NAME => {
                Self::try_new_unstable::<SentenceTerminal>(provider)
            }
            SoftDotted::NAME | SoftDotted::SHORT_NAME => {
                Self::try_new_unstable::<SoftDotted>(provider)
            }
            TerminalPunctuation::NAME | TerminalPunctuation::SHORT_NAME => {
                Self::try_new_unstable::<TerminalPunctuation>(provider)
            }
            UnifiedIdeograph::NAME | UnifiedIdeograph::SHORT_NAME => {
                Self::try_new_unstable::<UnifiedIdeograph>(provider)
            }
            Uppercase::NAME | Uppercase::SHORT_NAME => {
                Self::try_new_unstable::<Uppercase>(provider)
            }
            VariationSelector::NAME | VariationSelector::SHORT_NAME => {
                Self::try_new_unstable::<VariationSelector>(provider)
            }
            WhiteSpace::NAME | WhiteSpace::SHORT_NAME => {
                Self::try_new_unstable::<WhiteSpace>(provider)
            }
            XidContinue::NAME | XidContinue::SHORT_NAME => {
                Self::try_new_unstable::<XidContinue>(provider)
            }
            XidStart::NAME | XidStart::SHORT_NAME => Self::try_new_unstable::<XidStart>(provider),
            // Not an ECMA-262 property
            _ => return None,
        })
    }
}
