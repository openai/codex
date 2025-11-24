// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::codepointtrie::CodePointMapRange;

/// This is an iterator that coalesces adjacent ranges in an iterator over code
/// point ranges
pub(crate) struct RangeListIteratorCoalescer<I, T> {
    iter: I,
    peek: Option<CodePointMapRange<T>>,
}

impl<I, T: Eq> RangeListIteratorCoalescer<I, T>
where
    I: Iterator<Item = CodePointMapRange<T>>,
{
    pub fn new(iter: I) -> Self {
        Self { iter, peek: None }
    }
}

impl<I, T: Eq> Iterator for RangeListIteratorCoalescer<I, T>
where
    I: Iterator<Item = CodePointMapRange<T>>,
{
    type Item = CodePointMapRange<T>;

    fn next(&mut self) -> Option<Self::Item> {
        // Get the initial range we're working with: either a leftover
        // range from last time, or the next range
        let mut ret = if let Some(peek) = self.peek.take() {
            peek
        } else if let Some(next) = self.iter.next() {
            next
        } else {
            // No ranges, exit early
            return None;
        };

        // Keep pulling ranges
        #[expect(clippy::while_let_on_iterator)]
        // can't move the iterator, also we want it to be explicit that we're not draining the iterator
        while let Some(next) = self.iter.next() {
            if *next.range.start() == ret.range.end() + 1 && next.value == ret.value {
                // Range has no gap, coalesce
                ret.range = *ret.range.start()..=*next.range.end();
            } else {
                // Range has a gap, return what we have so far, update
                // peek
                self.peek = Some(next);
                return Some(ret);
            }
        }

        // Ran out of elements, exit
        Some(ret)
    }
}

#[cfg(test)]
mod tests {
    use core::fmt::Debug;
    use icu::collections::codepointinvlist::CodePointInversionListBuilder;
    use icu::properties::props::{BinaryProperty, EnumeratedProperty};
    use icu::properties::{CodePointMapData, CodePointSetData};

    fn test_set<P: BinaryProperty>(name: &str) {
        let mut builder = CodePointInversionListBuilder::new();
        let mut builder_complement = CodePointInversionListBuilder::new();

        for range in CodePointSetData::new::<P>().iter_ranges() {
            builder.add_range32(range)
        }

        for range in CodePointSetData::new::<P>().iter_ranges_complemented() {
            builder_complement.add_range32(range)
        }

        builder.complement();
        let set1 = builder.build();
        let set2 = builder_complement.build();
        assert_eq!(set1, set2, "Set {name} failed to complement correctly");
    }

    fn test_map<T: EnumeratedProperty + Debug>(value: T, name: &str) {
        let mut builder = CodePointInversionListBuilder::new();
        let mut builder_complement = CodePointInversionListBuilder::new();

        for range in CodePointMapData::<T>::new().iter_ranges_for_value(value) {
            builder.add_range32(range)
        }

        for range in CodePointMapData::<T>::new().iter_ranges_for_value_complemented(value) {
            builder_complement.add_range32(range)
        }

        builder.complement();
        let set1 = builder.build();
        let set2 = builder_complement.build();
        assert_eq!(
            set1, set2,
            "Map {name} failed to complement correctly with value {value:?}"
        );
    }

    #[test]
    fn test_complement_sets() {
        use icu::properties::props::*;
        // Stress test the RangeListIteratorComplementer logic by ensuring it works for
        // a whole bunch of binary properties
        test_set::<AsciiHexDigit>("ASCII_Hex_Digit");
        test_set::<Alnum>("Alnum");
        test_set::<Alphabetic>("Alphabetic");
        test_set::<BidiControl>("Bidi_Control");
        test_set::<BidiMirrored>("Bidi_Mirrored");
        test_set::<Blank>("Blank");
        test_set::<Cased>("Cased");
        test_set::<CaseIgnorable>("Case_Ignorable");
        test_set::<FullCompositionExclusion>("Full_Composition_Exclusion");
        test_set::<ChangesWhenCasefolded>("Changes_When_Casefolded");
        test_set::<ChangesWhenCasemapped>("Changes_When_Casemapped");
        test_set::<ChangesWhenNfkcCasefolded>("Changes_When_NFKC_Casefolded");
        test_set::<ChangesWhenLowercased>("Changes_When_Lowercased");
        test_set::<ChangesWhenTitlecased>("Changes_When_Titlecased");
        test_set::<ChangesWhenUppercased>("Changes_When_Uppercased");
        test_set::<Dash>("Dash");
        test_set::<Deprecated>("Deprecated");
        test_set::<DefaultIgnorableCodePoint>("Default_Ignorable_Code_Point");
        test_set::<Diacritic>("Diacritic");
        test_set::<EmojiModifierBase>("Emoji_Modifier_Base");
        test_set::<EmojiComponent>("Emoji_Component");
        test_set::<EmojiModifier>("Emoji_Modifier");
        test_set::<Emoji>("Emoji");
        test_set::<EmojiPresentation>("Emoji_Presentation");
        test_set::<Extender>("Extender");
        test_set::<ExtendedPictographic>("Extended_Pictographic");
        test_set::<Graph>("Graph");
        test_set::<GraphemeBase>("Grapheme_Base");
        test_set::<GraphemeExtend>("Grapheme_Extend");
        test_set::<GraphemeLink>("Grapheme_Link");
        test_set::<HexDigit>("Hex_Digit");
        test_set::<Hyphen>("Hyphen");
        test_set::<IdContinue>("Id_Continue");
        test_set::<Ideographic>("Ideographic");
        test_set::<IdStart>("Id_Start");
        test_set::<IdsBinaryOperator>("Ids_Binary_Operator");
        test_set::<IdsTrinaryOperator>("Ids_Trinary_Operator");
        test_set::<JoinControl>("Join_Control");
        test_set::<LogicalOrderException>("Logical_Order_Exception");
        test_set::<Lowercase>("Lowercase");
        test_set::<Math>("Math");
        test_set::<NoncharacterCodePoint>("Noncharacter_Code_Point");
        test_set::<NfcInert>("NFC_Inert");
        test_set::<NfdInert>("NFD_Inert");
        test_set::<NfkcInert>("NFKC_Inert");
        test_set::<NfkdInert>("NFKD_Inert");
        test_set::<PatternSyntax>("Pattern_Syntax");
        test_set::<PatternWhiteSpace>("Pattern_White_Space");
        test_set::<PrependedConcatenationMark>("Prepended_Concatenation_Mark");
        test_set::<Print>("Print");
        test_set::<QuotationMark>("Quotation_Mark");
        test_set::<Radical>("Radical");
        test_set::<RegionalIndicator>("Regional_Indicator");
        test_set::<SoftDotted>("Soft_Dotted");
        test_set::<SegmentStarter>("Segment_Starter");
        test_set::<CaseSensitive>("Case_Sensitive");
        test_set::<SentenceTerminal>("Sentence_Terminal");
        test_set::<TerminalPunctuation>("Terminal_Punctuation");
        test_set::<UnifiedIdeograph>("Unified_Ideograph");
        test_set::<Uppercase>("Uppercase");
        test_set::<VariationSelector>("Variation_Selector");
        test_set::<WhiteSpace>("White_Space");
        test_set::<Xdigit>("Xdigit");
        test_set::<XidContinue>("XID_Continue");
        test_set::<XidStart>("XID_Start");
    }

    #[test]
    fn test_complement_maps() {
        use icu::properties::props::{GeneralCategory, Script};
        test_map(GeneralCategory::UppercaseLetter, "gc");
        test_map(GeneralCategory::OtherPunctuation, "gc");
        test_map(Script::Devanagari, "script");
        test_map(Script::Latin, "script");
        test_map(Script::Common, "script");
    }
}
