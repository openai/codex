// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::cmp;
use core::iter::Filter;

// All of the logic for forward iteration over sentences
mod fwd {
    use crate::tables::sentence::SentenceCat;
    use core::cmp;

    // Describe a parsed part of source string as described in this table:
    // https://unicode.org/reports/tr29/#Default_Sentence_Boundaries
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum StatePart {
        Sot,
        Eot,
        Other,
        CR,
        LF,
        Sep,
        ATerm,
        UpperLower,
        ClosePlus,
        SpPlus,
        STerm,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SentenceBreaksState(pub [StatePart; 4]);

    const INITIAL_STATE: SentenceBreaksState = SentenceBreaksState([
        StatePart::Sot,
        StatePart::Sot,
        StatePart::Sot,
        StatePart::Sot,
    ]);

    #[derive(Debug, Clone)]
    pub struct SentenceBreaks<'a> {
        pub string: &'a str,
        pos: usize,
        state: SentenceBreaksState,
    }

    impl SentenceBreaksState {
        // Attempt to advance the internal state by one part
        // Whitespace and some punctutation will be collapsed
        fn next(&self, cat: SentenceCat) -> SentenceBreaksState {
            let &SentenceBreaksState(parts) = self;
            let parts = match (parts[3], cat) {
                (StatePart::ClosePlus, SentenceCat::SC_Close) => parts,
                (StatePart::SpPlus, SentenceCat::SC_Sp) => parts,
                _ => [
                    parts[1],
                    parts[2],
                    parts[3],
                    match cat {
                        SentenceCat::SC_CR => StatePart::CR,
                        SentenceCat::SC_LF => StatePart::LF,
                        SentenceCat::SC_Sep => StatePart::Sep,
                        SentenceCat::SC_ATerm => StatePart::ATerm,
                        SentenceCat::SC_Upper | SentenceCat::SC_Lower => StatePart::UpperLower,
                        SentenceCat::SC_Close => StatePart::ClosePlus,
                        SentenceCat::SC_Sp => StatePart::SpPlus,
                        SentenceCat::SC_STerm => StatePart::STerm,
                        _ => StatePart::Other,
                    },
                ],
            };
            SentenceBreaksState(parts)
        }

        fn end(&self) -> SentenceBreaksState {
            let &SentenceBreaksState(parts) = self;
            SentenceBreaksState([parts[1], parts[2], parts[3], StatePart::Eot])
        }

        // Helper function to check if state head matches a single `StatePart`
        fn match1(&self, part: StatePart) -> bool {
            let &SentenceBreaksState(parts) = self;
            part == parts[3]
        }

        // Helper function to check if first two `StateParts` in state match
        // the given two
        fn match2(&self, part1: StatePart, part2: StatePart) -> bool {
            let &SentenceBreaksState(parts) = self;
            part1 == parts[2] && part2 == parts[3]
        }
    }

    // https://unicode.org/reports/tr29/#SB8
    // TODO cache this, it is currently quadratic
    fn match_sb8(state: &SentenceBreaksState, ahead: &str) -> bool {
        let &SentenceBreaksState(parts) = state;
        let mut idx = if parts[3] == StatePart::SpPlus { 2 } else { 3 };
        if parts[idx] == StatePart::ClosePlus {
            idx -= 1
        }

        if parts[idx] == StatePart::ATerm {
            use crate::tables::sentence as se;

            for next_char in ahead.chars() {
                //( ¬(OLetter | Upper | Lower | ParaSep | SATerm) )* Lower
                match se::sentence_category(next_char).2 {
                    se::SC_Lower => return true,
                    se::SC_OLetter
                    | se::SC_Upper
                    | se::SC_Sep
                    | se::SC_CR
                    | se::SC_LF
                    | se::SC_STerm
                    | se::SC_ATerm => return false,
                    _ => continue,
                }
            }
        }

        false
    }

    // https://unicode.org/reports/tr29/#SB8a
    fn match_sb8a(state: &SentenceBreaksState) -> bool {
        // SATerm Close* Sp*
        let &SentenceBreaksState(parts) = state;
        let mut idx = if parts[3] == StatePart::SpPlus { 2 } else { 3 };
        if parts[idx] == StatePart::ClosePlus {
            idx -= 1
        }
        parts[idx] == StatePart::STerm || parts[idx] == StatePart::ATerm
    }

    // https://unicode.org/reports/tr29/#SB9
    fn match_sb9(state: &SentenceBreaksState) -> bool {
        // SATerm Close*
        let &SentenceBreaksState(parts) = state;
        let idx = if parts[3] == StatePart::ClosePlus {
            2
        } else {
            3
        };
        parts[idx] == StatePart::STerm || parts[idx] == StatePart::ATerm
    }

    // https://unicode.org/reports/tr29/#SB11
    fn match_sb11(state: &SentenceBreaksState) -> bool {
        // SATerm Close* Sp* ParaSep?
        let &SentenceBreaksState(parts) = state;
        let mut idx = match parts[3] {
            StatePart::Sep | StatePart::CR | StatePart::LF => 2,
            _ => 3,
        };

        if parts[idx] == StatePart::SpPlus {
            idx -= 1
        }
        if parts[idx] == StatePart::ClosePlus {
            idx -= 1
        }

        parts[idx] == StatePart::STerm || parts[idx] == StatePart::ATerm
    }

    impl<'a> Iterator for SentenceBreaks<'a> {
        // Returns the index of the character which follows a break
        type Item = usize;

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            let slen = self.string.len();
            // A sentence could be one character
            (cmp::min(slen, 2), Some(slen + 1))
        }

        #[inline]
        fn next(&mut self) -> Option<usize> {
            use crate::tables::sentence as se;

            for next_char in self.string[self.pos..].chars() {
                let position_before = self.pos;
                let state_before = self.state.clone();

                let next_cat = se::sentence_category(next_char).2;

                self.pos += next_char.len_utf8();
                self.state = self.state.next(next_cat);

                match next_cat {
                    // SB1 https://unicode.org/reports/tr29/#SB1
                    _ if state_before.match1(StatePart::Sot) => return Some(position_before),

                    // SB2 is handled when inner iterator (chars) is finished

                    // SB3 https://unicode.org/reports/tr29/#SB3
                    SentenceCat::SC_LF if state_before.match1(StatePart::CR) => continue,

                    // SB4 https://unicode.org/reports/tr29/#SB4
                    _ if state_before.match1(StatePart::Sep)
                        || state_before.match1(StatePart::CR)
                        || state_before.match1(StatePart::LF) =>
                    {
                        return Some(position_before)
                    }

                    // SB5 https://unicode.org/reports/tr29/#SB5
                    SentenceCat::SC_Extend | SentenceCat::SC_Format => self.state = state_before,

                    // SB6 https://unicode.org/reports/tr29/#SB6
                    SentenceCat::SC_Numeric if state_before.match1(StatePart::ATerm) => continue,

                    // SB7 https://unicode.org/reports/tr29/#SB7
                    SentenceCat::SC_Upper
                        if state_before.match2(StatePart::UpperLower, StatePart::ATerm) =>
                    {
                        continue
                    }

                    // SB8 https://unicode.org/reports/tr29/#SB8
                    _ if match_sb8(&state_before, &self.string[position_before..]) => continue,

                    // SB8a https://unicode.org/reports/tr29/#SB8a
                    SentenceCat::SC_SContinue | SentenceCat::SC_STerm | SentenceCat::SC_ATerm
                        if match_sb8a(&state_before) =>
                    {
                        continue
                    }

                    // SB9 https://unicode.org/reports/tr29/#SB9
                    SentenceCat::SC_Close
                    | SentenceCat::SC_Sp
                    | SentenceCat::SC_Sep
                    | SentenceCat::SC_CR
                    | SentenceCat::SC_LF
                        if match_sb9(&state_before) =>
                    {
                        continue
                    }

                    // SB10 https://unicode.org/reports/tr29/#SB10
                    SentenceCat::SC_Sp
                    | SentenceCat::SC_Sep
                    | SentenceCat::SC_CR
                    | SentenceCat::SC_LF
                        if match_sb8a(&state_before) =>
                    {
                        continue
                    }

                    // SB11 https://unicode.org/reports/tr29/#SB11
                    _ if match_sb11(&state_before) => return Some(position_before),

                    // SB998 https://unicode.org/reports/tr29/#SB998
                    _ => continue,
                }
            }

            // SB2 https://unicode.org/reports/tr29/#SB2
            if self.state.match1(StatePart::Sot) || self.state.match1(StatePart::Eot) {
                None
            } else {
                self.state = self.state.end();
                Some(self.pos)
            }
        }
    }

    pub fn new_sentence_breaks(source: &str) -> SentenceBreaks<'_> {
        SentenceBreaks {
            string: source,
            pos: 0,
            state: INITIAL_STATE,
        }
    }
}

/// An iterator over the substrings of a string which, after splitting the string on
/// [sentence boundaries](http://www.unicode.org/reports/tr29/#Sentence_Boundaries),
/// contain any characters with the
/// [Alphabetic](http://unicode.org/reports/tr44/#Alphabetic)
/// property, or with
/// [General_Category=Number](http://unicode.org/reports/tr44/#General_Category_Values).
///
/// This struct is created by the [`unicode_sentences`] method on the [`UnicodeSegmentation`]
/// trait. See its documentation for more.
///
/// [`unicode_sentences`]: trait.UnicodeSegmentation.html#tymethod.unicode_sentences
/// [`UnicodeSegmentation`]: trait.UnicodeSegmentation.html
#[derive(Debug, Clone)]
pub struct UnicodeSentences<'a> {
    inner: Filter<USentenceBounds<'a>, fn(&&str) -> bool>,
}

/// External iterator for a string's
/// [sentence boundaries](http://www.unicode.org/reports/tr29/#Sentence_Boundaries).
///
/// This struct is created by the [`split_sentence_bounds`] method on the [`UnicodeSegmentation`]
/// trait. See its documentation for more.
///
/// [`split_sentence_bounds`]: trait.UnicodeSegmentation.html#tymethod.split_sentence_bounds
/// [`UnicodeSegmentation`]: trait.UnicodeSegmentation.html
#[derive(Debug, Clone)]
pub struct USentenceBounds<'a> {
    iter: fwd::SentenceBreaks<'a>,
    sentence_start: Option<usize>,
}

/// External iterator for sentence boundaries and byte offsets.
///
/// This struct is created by the [`split_sentence_bound_indices`] method on the
/// [`UnicodeSegmentation`] trait. See its documentation for more.
///
/// [`split_sentence_bound_indices`]: trait.UnicodeSegmentation.html#tymethod.split_sentence_bound_indices
/// [`UnicodeSegmentation`]: trait.UnicodeSegmentation.html
#[derive(Debug, Clone)]
pub struct USentenceBoundIndices<'a> {
    start_offset: usize,
    iter: USentenceBounds<'a>,
}

#[inline]
pub fn new_sentence_bounds(source: &str) -> USentenceBounds<'_> {
    USentenceBounds {
        iter: fwd::new_sentence_breaks(source),
        sentence_start: None,
    }
}

#[inline]
pub fn new_sentence_bound_indices(source: &str) -> USentenceBoundIndices<'_> {
    USentenceBoundIndices {
        start_offset: source.as_ptr() as usize,
        iter: new_sentence_bounds(source),
    }
}

#[inline]
pub fn new_unicode_sentences(s: &str) -> UnicodeSentences<'_> {
    use super::UnicodeSegmentation;
    use crate::tables::util::is_alphanumeric;

    fn has_alphanumeric(s: &&str) -> bool {
        s.chars().any(is_alphanumeric)
    }
    let has_alphanumeric: fn(&&str) -> bool = has_alphanumeric; // coerce to fn pointer

    UnicodeSentences {
        inner: s.split_sentence_bounds().filter(has_alphanumeric),
    }
}

impl<'a> Iterator for UnicodeSentences<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        self.inner.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a> Iterator for USentenceBounds<'a> {
    type Item = &'a str;

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();
        (cmp::max(0, lower - 1), upper.map(|u| cmp::max(0, u - 1)))
    }

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        if self.sentence_start.is_none() {
            if let Some(start_pos) = self.iter.next() {
                self.sentence_start = Some(start_pos)
            } else {
                return None;
            }
        }

        if let Some(break_pos) = self.iter.next() {
            let start_pos = self.sentence_start.unwrap();
            let sentence = &self.iter.string[start_pos..break_pos];
            self.sentence_start = Some(break_pos);
            Some(sentence)
        } else {
            None
        }
    }
}

impl<'a> Iterator for USentenceBoundIndices<'a> {
    type Item = (usize, &'a str);

    #[inline]
    fn next(&mut self) -> Option<(usize, &'a str)> {
        self.iter
            .next()
            .map(|s| (s.as_ptr() as usize - self.start_offset, s))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}
