// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::iter::FromIterator;
use core::{
    convert::TryFrom,
    ops::{Range, RangeBounds, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive},
};

use super::RangeError;
use crate::codepointinvlist::utils::deconstruct_range;
use crate::codepointinvlist::CodePointInversionList;
use crate::codepointinvlist::CodePointInversionListBuilder;
use potential_utf::PotentialCodePoint;
use zerovec::ZeroVec;

fn try_from_range<'data>(
    range: impl RangeBounds<char>,
) -> Result<CodePointInversionList<'data>, RangeError> {
    let (from, till) = deconstruct_range(range);
    if from < till {
        let set = [
            PotentialCodePoint::from_u24(from),
            PotentialCodePoint::from_u24(till),
        ];
        let inv_list: ZeroVec<PotentialCodePoint> = ZeroVec::alloc_from_slice(&set);
        #[expect(clippy::unwrap_used)] // valid
        Ok(CodePointInversionList::try_from_inversion_list(inv_list).unwrap())
    } else {
        Err(RangeError(from, till))
    }
}

impl TryFrom<Range<char>> for CodePointInversionList<'_> {
    type Error = RangeError;

    fn try_from(range: Range<char>) -> Result<Self, Self::Error> {
        try_from_range(range)
    }
}

impl TryFrom<RangeFrom<char>> for CodePointInversionList<'_> {
    type Error = RangeError;

    fn try_from(range: RangeFrom<char>) -> Result<Self, Self::Error> {
        try_from_range(range)
    }
}

impl TryFrom<RangeFull> for CodePointInversionList<'_> {
    type Error = RangeError;

    fn try_from(_: RangeFull) -> Result<Self, Self::Error> {
        Ok(Self::all())
    }
}

impl TryFrom<RangeInclusive<char>> for CodePointInversionList<'_> {
    type Error = RangeError;

    fn try_from(range: RangeInclusive<char>) -> Result<Self, Self::Error> {
        try_from_range(range)
    }
}

impl TryFrom<RangeTo<char>> for CodePointInversionList<'_> {
    type Error = RangeError;

    fn try_from(range: RangeTo<char>) -> Result<Self, Self::Error> {
        try_from_range(range)
    }
}

impl TryFrom<RangeToInclusive<char>> for CodePointInversionList<'_> {
    type Error = RangeError;

    fn try_from(range: RangeToInclusive<char>) -> Result<Self, Self::Error> {
        try_from_range(range)
    }
}

impl FromIterator<RangeInclusive<u32>> for CodePointInversionList<'_> {
    fn from_iter<I: IntoIterator<Item = RangeInclusive<u32>>>(iter: I) -> Self {
        let mut builder = CodePointInversionListBuilder::new();
        for range in iter {
            builder.add_range32(range);
        }
        builder.build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codepointinvlist::CodePointInversionList;
    use core::{char, convert::TryFrom};

    #[test]
    fn test_try_from_range() {
        let check: Vec<char> = CodePointInversionList::try_from('A'..'B')
            .unwrap()
            .iter_chars()
            .collect();
        assert_eq!(vec!['A'], check);
    }

    #[test]
    fn test_try_from_range_error() {
        let check = CodePointInversionList::try_from('A'..'A');
        assert!(matches!(check, Err(RangeError(65, 65))));
    }

    #[test]
    fn test_try_from_range_inclusive() {
        let check: Vec<char> = CodePointInversionList::try_from('A'..='A')
            .unwrap()
            .iter_chars()
            .collect();
        assert_eq!(vec!['A'], check);
    }

    #[test]
    fn test_try_from_range_inclusive_err() {
        let check = CodePointInversionList::try_from('B'..'A');
        assert!(matches!(check, Err(RangeError(66, 65))));
    }

    #[test]
    fn test_try_from_range_from() {
        let uset = CodePointInversionList::try_from('A'..).unwrap();
        let check: usize = uset.size();
        let expected: usize = (char::MAX as usize) + 1 - 65;
        assert_eq!(expected, check);
    }

    #[test]
    fn test_try_from_range_to() {
        let uset = CodePointInversionList::try_from(..'A').unwrap();
        let check: usize = uset.size();
        let expected: usize = 65;
        assert_eq!(expected, check);
    }

    #[test]
    fn test_try_from_range_to_err() {
        let check = CodePointInversionList::try_from(..(0x0 as char));
        assert!(matches!(check, Err(RangeError(0, 0))));
    }

    #[test]
    fn test_try_from_range_to_inclusive() {
        let uset = CodePointInversionList::try_from(..='A').unwrap();
        let check: usize = uset.size();
        let expected: usize = 66;
        assert_eq!(expected, check);
    }

    #[test]
    fn test_try_from_range_full() {
        let uset = CodePointInversionList::try_from(..).unwrap();
        let check: usize = uset.size();
        let expected: usize = (char::MAX as usize) + 1;
        assert_eq!(expected, check);
    }

    #[test]
    fn test_from_range_iterator() {
        let ranges = [
            0..=0x3FFF,
            0x4000..=0x7FFF,
            0x8000..=0xBFFF,
            0xC000..=0xFFFF,
        ];
        let expected =
            CodePointInversionList::try_from_u32_inversion_list_slice(&[0x0, 0x1_0000]).unwrap();
        let actual = CodePointInversionList::from_iter(ranges);
        assert_eq!(expected, actual);
    }
}
