//! Utilities to generate bitmasks.

#![doc(hidden)]

/// Generate a bitwise mask for the lower `n` bits.
///
/// # Examples
///
/// ```rust
/// # use minimal_lexical::mask::lower_n_mask;
/// # pub fn main() {
/// assert_eq!(lower_n_mask(2), 0b11);
/// # }
/// ```
#[inline]
pub fn lower_n_mask(n: u64) -> u64 {
    debug_assert!(n <= 64, "lower_n_mask() overflow in shl.");

    match n == 64 {
        // u64::MAX for older Rustc versions.
        true => 0xffff_ffff_ffff_ffff,
        false => (1 << n) - 1,
    }
}

/// Calculate the halfway point for the lower `n` bits.
///
/// # Examples
///
/// ```rust
/// # use minimal_lexical::mask::lower_n_halfway;
/// # pub fn main() {
/// assert_eq!(lower_n_halfway(2), 0b10);
/// # }
/// ```
#[inline]
pub fn lower_n_halfway(n: u64) -> u64 {
    debug_assert!(n <= 64, "lower_n_halfway() overflow in shl.");

    match n == 0 {
        true => 0,
        false => nth_bit(n - 1),
    }
}

/// Calculate a scalar factor of 2 above the halfway point.
///
/// # Examples
///
/// ```rust
/// # use minimal_lexical::mask::nth_bit;
/// # pub fn main() {
/// assert_eq!(nth_bit(2), 0b100);
/// # }
/// ```
#[inline]
pub fn nth_bit(n: u64) -> u64 {
    debug_assert!(n < 64, "nth_bit() overflow in shl.");
    1 << n
}
