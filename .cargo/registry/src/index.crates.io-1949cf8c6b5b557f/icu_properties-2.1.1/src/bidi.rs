// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::{props::EnumeratedProperty, provider::PropertyEnumBidiMirroringGlyphV1};
use icu_collections::codepointtrie::TrieValue;
use zerovec::ule::{AsULE, RawBytesULE};

/// This is a bitpacked combination of the `Bidi_Mirroring_Glyph`,
/// `Bidi_Mirrored`, and `Bidi_Paired_Bracket_Type` properties.
#[derive(Debug, Eq, PartialEq, Clone, Copy, Default)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_properties::props))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[allow(clippy::exhaustive_structs)] // needed for baked construction
pub struct BidiMirroringGlyph {
    /// The mirroring glyph
    pub mirroring_glyph: Option<char>,
    /// Whether the glyph is mirrored
    pub mirrored: bool,
    /// The paired bracket type
    pub paired_bracket_type: BidiPairedBracketType,
}

impl EnumeratedProperty for BidiMirroringGlyph {
    type DataMarker = PropertyEnumBidiMirroringGlyphV1;
    #[cfg(feature = "compiled_data")]
    const SINGLETON: &'static crate::provider::PropertyCodePointMap<'static, Self> =
        crate::provider::Baked::SINGLETON_PROPERTY_ENUM_BIDI_MIRRORING_GLYPH_V1;
    const NAME: &'static [u8] = b"Bidi_Mirroring_Glyph";
    const SHORT_NAME: &'static [u8] = b"Bidi_Mirroring_Glyph";
}

impl crate::private::Sealed for BidiMirroringGlyph {}

impl AsULE for BidiMirroringGlyph {
    type ULE = zerovec::ule::RawBytesULE<3>;

    fn to_unaligned(self) -> Self::ULE {
        let [a, b, c, _] = TrieValue::to_u32(self).to_le_bytes();
        RawBytesULE([a, b, c])
    }
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        let [a, b, c] = unaligned.0;
        TrieValue::try_from_u32(u32::from_le_bytes([a, b, c, 0])).unwrap_or_default()
    }
}

/// The enum represents Bidi_Paired_Bracket_Type.
///
/// It does not implement [`EnumeratedProperty`], instead it can be obtained
/// through the bitpacked [`BidiMirroringGlyph`] property.
///
/// If you have a use case this property without also needing the [`BidiMirroringGlyph`]
/// property, and need to optimize data size, please file an issue.
#[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
#[cfg_attr(feature = "datagen", derive(serde::Serialize, databake::Bake))]
#[cfg_attr(feature = "datagen", databake(path = icu_properties::props))]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[non_exhaustive]
pub enum BidiPairedBracketType {
    /// Represents Bidi_Paired_Bracket_Type=Open.
    Open,
    /// Represents Bidi_Paired_Bracket_Type=Close.
    Close,
    /// Represents Bidi_Paired_Bracket_Type=None.
    #[default]
    None,
}

/// Implements [`unicode_bidi::BidiDataSource`] on [`CodePointMapDataBorrowed<BidiClass>`](crate::CodePointMapDataBorrowed).
///
/// ✨ *Enabled with the `unicode_bidi` Cargo feature.*
///
/// # Examples
///
///```
/// use icu::properties::CodePointMapData;
/// use icu::properties::props::BidiClass;
/// use unicode_bidi::BidiInfo;
///
/// // This example text is defined using `concat!` because some browsers
/// // and text editors have trouble displaying bidi strings.
/// let text =  concat!["א", // RTL#1
///                     "ב", // RTL#2
///                     "ג", // RTL#3
///                     "a", // LTR#1
///                     "b", // LTR#2
///                     "c", // LTR#3
///                     ]; //
///
///
/// let bidi_map = CodePointMapData::<BidiClass>::new();
///
/// // Resolve embedding levels within the text.  Pass `None` to detect the
/// // paragraph level automatically.
/// let bidi_info = BidiInfo::new_with_data_source(&bidi_map, text, None);
///
/// // This paragraph has embedding level 1 because its first strong character is RTL.
/// assert_eq!(bidi_info.paragraphs.len(), 1);
/// let para = &bidi_info.paragraphs[0];
/// assert_eq!(para.level.number(), 1);
/// assert!(para.level.is_rtl());
///
/// // Re-ordering is done after wrapping each paragraph into a sequence of
/// // lines. For this example, I'll just use a single line that spans the
/// // entire paragraph.
/// let line = para.range.clone();
///
/// let display = bidi_info.reorder_line(para, line);
/// assert_eq!(display, concat!["a", // LTR#1
///                             "b", // LTR#2
///                             "c", // LTR#3
///                             "ג", // RTL#3
///                             "ב", // RTL#2
///                             "א", // RTL#1
///                             ]);
/// ```
#[cfg(feature = "unicode_bidi")]
impl unicode_bidi::data_source::BidiDataSource
    for crate::CodePointMapDataBorrowed<'_, crate::props::BidiClass>
{
    fn bidi_class(&self, c: char) -> unicode_bidi::BidiClass {
        use crate::props::BidiClass;
        match self.get(c) {
            BidiClass::LeftToRight => unicode_bidi::BidiClass::L,
            BidiClass::RightToLeft => unicode_bidi::BidiClass::R,
            BidiClass::EuropeanNumber => unicode_bidi::BidiClass::EN,
            BidiClass::EuropeanSeparator => unicode_bidi::BidiClass::ES,
            BidiClass::EuropeanTerminator => unicode_bidi::BidiClass::ET,
            BidiClass::ArabicNumber => unicode_bidi::BidiClass::AN,
            BidiClass::CommonSeparator => unicode_bidi::BidiClass::CS,
            BidiClass::ParagraphSeparator => unicode_bidi::BidiClass::B,
            BidiClass::SegmentSeparator => unicode_bidi::BidiClass::S,
            BidiClass::WhiteSpace => unicode_bidi::BidiClass::WS,
            BidiClass::OtherNeutral => unicode_bidi::BidiClass::ON,
            BidiClass::LeftToRightEmbedding => unicode_bidi::BidiClass::LRE,
            BidiClass::LeftToRightOverride => unicode_bidi::BidiClass::LRO,
            BidiClass::ArabicLetter => unicode_bidi::BidiClass::AL,
            BidiClass::RightToLeftEmbedding => unicode_bidi::BidiClass::RLE,
            BidiClass::RightToLeftOverride => unicode_bidi::BidiClass::RLO,
            BidiClass::PopDirectionalFormat => unicode_bidi::BidiClass::PDF,
            BidiClass::NonspacingMark => unicode_bidi::BidiClass::NSM,
            BidiClass::BoundaryNeutral => unicode_bidi::BidiClass::BN,
            BidiClass::FirstStrongIsolate => unicode_bidi::BidiClass::FSI,
            BidiClass::LeftToRightIsolate => unicode_bidi::BidiClass::LRI,
            BidiClass::RightToLeftIsolate => unicode_bidi::BidiClass::RLI,
            BidiClass::PopDirectionalIsolate => unicode_bidi::BidiClass::PDI,
            // This must not happen.
            _ => unicode_bidi::BidiClass::ON,
        }
    }
}
