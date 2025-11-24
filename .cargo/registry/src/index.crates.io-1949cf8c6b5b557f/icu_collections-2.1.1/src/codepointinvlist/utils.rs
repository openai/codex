// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::{
    char,
    ops::{Bound::*, RangeBounds},
};
use potential_utf::PotentialCodePoint;
use zerovec::ule::AsULE;
use zerovec::ZeroVec;

/// Returns whether the vector is sorted ascending non inclusive, of even length,
/// and within the bounds of `0x0 -> 0x10FFFF + 1` inclusive.
#[expect(clippy::indexing_slicing)] // windows
#[expect(clippy::unwrap_used)] // by is_empty check
pub fn is_valid_zv(inv_list_zv: &ZeroVec<'_, PotentialCodePoint>) -> bool {
    inv_list_zv.is_empty()
        || (inv_list_zv.len() % 2 == 0
            && inv_list_zv.as_ule_slice().windows(2).all(|chunk| {
                <PotentialCodePoint as AsULE>::from_unaligned(chunk[0])
                    < <PotentialCodePoint as AsULE>::from_unaligned(chunk[1])
            })
            && u32::from(inv_list_zv.last().unwrap()) <= char::MAX as u32 + 1)
}

/// Returns start (inclusive) and end (exclusive) bounds of [`RangeBounds`]
pub fn deconstruct_range<T>(range: impl RangeBounds<T>) -> (u32, u32)
where
    T: Into<u32> + Copy,
{
    let from = match range.start_bound() {
        Included(b) => (*b).into(),
        Excluded(_) => unreachable!(),
        Unbounded => 0,
    };
    let till = match range.end_bound() {
        Included(b) => (*b).into() + 1,
        Excluded(b) => (*b).into(),
        Unbounded => (char::MAX as u32) + 1,
    };
    (from, till)
}

#[cfg(test)]
mod tests {
    use super::{deconstruct_range, is_valid_zv, PotentialCodePoint};
    use core::char;
    use zerovec::ZeroVec;

    fn make_zv(slice: &[u32]) -> ZeroVec<'_, PotentialCodePoint> {
        slice
            .iter()
            .copied()
            .map(PotentialCodePoint::from_u24)
            .collect()
    }
    #[test]
    fn test_is_valid_zv() {
        let check = make_zv(&[0x2, 0x3, 0x4, 0x5]);
        assert!(is_valid_zv(&check));
    }

    #[test]
    fn test_is_valid_zv_empty() {
        let check = make_zv(&[]);
        assert!(is_valid_zv(&check));
    }

    #[test]
    fn test_is_valid_zv_overlapping() {
        let check = make_zv(&[0x2, 0x5, 0x4, 0x6]);
        assert!(!is_valid_zv(&check));
    }

    #[test]
    fn test_is_valid_zv_out_of_order() {
        let check = make_zv(&[0x5, 0x4, 0x5, 0x6, 0x7]);
        assert!(!is_valid_zv(&check));
    }

    #[test]
    fn test_is_valid_zv_duplicate() {
        let check = make_zv(&[0x1, 0x2, 0x3, 0x3, 0x5]);
        assert!(!is_valid_zv(&check));
    }

    #[test]
    fn test_is_valid_zv_odd() {
        let check = make_zv(&[0x1, 0x2, 0x3, 0x4, 0x5]);
        assert!(!is_valid_zv(&check));
    }

    #[test]
    fn test_is_valid_zv_out_of_range() {
        let check = make_zv(&[0x1, 0x2, 0x3, 0x4, (char::MAX as u32) + 1]);
        assert!(!is_valid_zv(&check));
    }

    // deconstruct_range

    #[test]
    fn test_deconstruct_range() {
        let expected = (0x41, 0x45);
        let check = deconstruct_range('A'..'E'); // Range
        assert_eq!(check, expected);
        let check = deconstruct_range('A'..='D'); // Range Inclusive
        assert_eq!(check, expected);
        let check = deconstruct_range('A'..); // Range From
        assert_eq!(check, (0x41, (char::MAX as u32) + 1));
        let check = deconstruct_range(..'A'); // Range To
        assert_eq!(check, (0x0, 0x41));
        let check = deconstruct_range(..='A'); // Range To Inclusive
        assert_eq!(check, (0x0, 0x42));
        let check = deconstruct_range::<char>(..); // Range Full
        assert_eq!(check, (0x0, (char::MAX as u32) + 1));
    }
}
