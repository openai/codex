// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::bidi::BidiMirroringGlyph;
use crate::props::{
    BidiClass, CanonicalCombiningClass, EastAsianWidth, GeneralCategory, GeneralCategoryGroup,
    GraphemeClusterBreak, HangulSyllableType, IndicConjunctBreak, IndicSyllabicCategory,
    JoiningType, LineBreak, Script, SentenceBreak, VerticalOrientation, WordBreak,
};
use crate::script::ScriptWithExt;
use core::convert::TryInto;
use core::num::TryFromIntError;
use zerovec::ule::{AsULE, RawBytesULE};

use icu_collections::codepointtrie::TrieValue;

use core::convert::TryFrom;

impl TrieValue for CanonicalCombiningClass {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u8::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

impl TrieValue for BidiClass {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u8::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

impl TrieValue for GeneralCategory {
    type TryFromU32Error = &'static str;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        // If the u32 is out of range, fall back to u8::MAX, which is out of range of the GeneralCategory enum.
        GeneralCategory::new_from_u8(i.try_into().unwrap_or(u8::MAX))
            .ok_or("Cannot parse GeneralCategory from integer")
    }

    fn to_u32(self) -> u32 {
        u32::from(self as u8)
    }
}

impl TrieValue for Script {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u16::try_from(i).map(Script)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

impl TrieValue for HangulSyllableType {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u8::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

impl TrieValue for ScriptWithExt {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u16::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

impl TrieValue for EastAsianWidth {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u8::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

impl TrieValue for LineBreak {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u8::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

impl TrieValue for GraphemeClusterBreak {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u8::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

impl TrieValue for WordBreak {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u8::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

impl TrieValue for SentenceBreak {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u8::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

impl TrieValue for IndicConjunctBreak {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u8::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

impl TrieValue for IndicSyllabicCategory {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u8::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

impl TrieValue for VerticalOrientation {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u8::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}

// GCG is not used inside tries, but it is used in the name lookup type, and we want
// to squeeze it into a u16 for storage. Its named mask values are specced so we can
// do this in code.
//
// This is done by:
// - Single-value masks are translated to their corresponding GeneralCategory values
// - we know all of the multi-value masks and we give them special values
// - Anything else goes to 0xFF00, though this code path shouldn't be hit unless working with malformed icuexportdata
//
// In the reverse direction, unknown values go to the empty mask, but this codepath should not be hit except
// with malformed ICU4X generated data.
impl AsULE for GeneralCategoryGroup {
    type ULE = RawBytesULE<2>;
    fn to_unaligned(self) -> Self::ULE {
        let value = gcg_to_packed_u16(self);
        value.to_unaligned()
    }
    fn from_unaligned(ule: Self::ULE) -> Self {
        let value = ule.as_unsigned_int();
        packed_u16_to_gcg(value)
    }
}

fn packed_u16_to_gcg(value: u16) -> GeneralCategoryGroup {
    match value {
        0xFFFF => GeneralCategoryGroup::CasedLetter,
        0xFFFE => GeneralCategoryGroup::Letter,
        0xFFFD => GeneralCategoryGroup::Mark,
        0xFFFC => GeneralCategoryGroup::Number,
        0xFFFB => GeneralCategoryGroup::Separator,
        0xFFFA => GeneralCategoryGroup::Other,
        0xFFF9 => GeneralCategoryGroup::Punctuation,
        0xFFF8 => GeneralCategoryGroup::Symbol,
        v if v < 32 => GeneralCategory::new_from_u8(v as u8)
            .map(|gc| gc.into())
            .unwrap_or(GeneralCategoryGroup(0)),
        // unknown values produce an empty mask
        _ => GeneralCategoryGroup(0),
    }
}

fn gcg_to_packed_u16(gcg: GeneralCategoryGroup) -> u16 {
    // if it's a single property, translate to that property
    if gcg.0.is_power_of_two() {
        // inverse operation of a bitshift
        gcg.0.trailing_zeros() as u16
    } else {
        match gcg {
            GeneralCategoryGroup::CasedLetter => 0xFFFF,
            GeneralCategoryGroup::Letter => 0xFFFE,
            GeneralCategoryGroup::Mark => 0xFFFD,
            GeneralCategoryGroup::Number => 0xFFFC,
            GeneralCategoryGroup::Separator => 0xFFFB,
            GeneralCategoryGroup::Other => 0xFFFA,
            GeneralCategoryGroup::Punctuation => 0xFFF9,
            GeneralCategoryGroup::Symbol => 0xFFF8,
            _ => 0xFF00, // random sentinel value
        }
    }
}

impl TrieValue for GeneralCategoryGroup {
    type TryFromU32Error = TryFromIntError;
    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        // Even though we're dealing with u32s here, TrieValue is about converting
        // trie storage types to the actual type. This type will always be a packed u16
        // in our case since the names map upcasts from u16
        u16::try_from(i).map(packed_u16_to_gcg)
    }

    fn to_u32(self) -> u32 {
        u32::from(gcg_to_packed_u16(self))
    }
}

impl TrieValue for BidiMirroringGlyph {
    type TryFromU32Error = u32;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        let code_point = i & 0x1FFFFF;
        let mirroring_glyph = if code_point == 0 {
            None
        } else {
            Some(char::try_from_u32(code_point).map_err(|_| i)?)
        };
        let mirrored = ((i >> 21) & 0x1) == 1;
        let paired_bracket_type = {
            let value = ((i >> 22) & 0x3) as u8;
            match value {
                0 => crate::bidi::BidiPairedBracketType::None,
                1 => crate::bidi::BidiPairedBracketType::Open,
                2 => crate::bidi::BidiPairedBracketType::Close,
                _ => return Err(i),
            }
        };
        Ok(Self {
            mirrored,
            mirroring_glyph,
            paired_bracket_type,
        })
    }

    fn to_u32(self) -> u32 {
        self.mirroring_glyph.unwrap_or_default() as u32
            | ((self.mirrored as u32) << 21)
            | (match self.paired_bracket_type {
                crate::bidi::BidiPairedBracketType::None => 0,
                crate::bidi::BidiPairedBracketType::Open => 1,
                crate::bidi::BidiPairedBracketType::Close => 2,
            } << 22)
    }
}

impl TrieValue for JoiningType {
    type TryFromU32Error = TryFromIntError;

    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
        u8::try_from(i).map(Self)
    }

    fn to_u32(self) -> u32 {
        u32::from(self.0)
    }
}
