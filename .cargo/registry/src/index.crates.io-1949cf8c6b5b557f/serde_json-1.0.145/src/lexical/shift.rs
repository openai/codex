// Adapted from https://github.com/Alexhuszagh/rust-lexical.

//! Bit-shift helpers.

use super::float::ExtendedFloat;
use core::mem;

// Shift extended-precision float right `shift` bytes.
#[inline]
pub(crate) fn shr(fp: &mut ExtendedFloat, shift: i32) {
    let bits: u64 = mem::size_of::<u64>() as u64 * 8;
    debug_assert!((shift as u64) < bits, "shr() overflow in shift right.");

    fp.mant >>= shift;
    fp.exp += shift;
}

// Shift extended-precision float right `shift` bytes.
//
// Accepts when the shift is the same as the type size, and
// sets the value to 0.
#[inline]
pub(crate) fn overflowing_shr(fp: &mut ExtendedFloat, shift: i32) {
    let bits: u64 = mem::size_of::<u64>() as u64 * 8;
    debug_assert!(
        (shift as u64) <= bits,
        "overflowing_shr() overflow in shift right."
    );

    fp.mant = if shift as u64 == bits {
        0
    } else {
        fp.mant >> shift
    };
    fp.exp += shift;
}

// Shift extended-precision float left `shift` bytes.
#[inline]
pub(crate) fn shl(fp: &mut ExtendedFloat, shift: i32) {
    let bits: u64 = mem::size_of::<u64>() as u64 * 8;
    debug_assert!((shift as u64) < bits, "shl() overflow in shift left.");

    fp.mant <<= shift;
    fp.exp -= shift;
}
