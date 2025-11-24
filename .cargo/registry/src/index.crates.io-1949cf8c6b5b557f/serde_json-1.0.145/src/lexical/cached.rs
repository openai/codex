// Adapted from https://github.com/Alexhuszagh/rust-lexical.

//! Cached powers trait for extended-precision floats.

use super::cached_float80;
use super::float::ExtendedFloat;

// POWERS

/// Precalculated powers that uses two-separate arrays for memory-efficiency.
#[doc(hidden)]
pub(crate) struct ExtendedFloatArray {
    // Pre-calculated mantissa for the powers.
    pub mant: &'static [u64],
    // Pre-calculated binary exponents for the powers.
    pub exp: &'static [i32],
}

/// Allow indexing of values without bounds checking
impl ExtendedFloatArray {
    #[inline]
    pub fn get_extended_float(&self, index: usize) -> ExtendedFloat {
        let mant = self.mant[index];
        let exp = self.exp[index];
        ExtendedFloat { mant, exp }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.mant.len()
    }
}

// MODERATE PATH POWERS

/// Precalculated powers of base N for the moderate path.
#[doc(hidden)]
pub(crate) struct ModeratePathPowers {
    // Pre-calculated small powers.
    pub small: ExtendedFloatArray,
    // Pre-calculated large powers.
    pub large: ExtendedFloatArray,
    /// Pre-calculated small powers as 64-bit integers
    pub small_int: &'static [u64],
    // Step between large powers and number of small powers.
    pub step: i32,
    // Exponent bias for the large powers.
    pub bias: i32,
}

/// Allow indexing of values without bounds checking
impl ModeratePathPowers {
    #[inline]
    pub fn get_small(&self, index: usize) -> ExtendedFloat {
        self.small.get_extended_float(index)
    }

    #[inline]
    pub fn get_large(&self, index: usize) -> ExtendedFloat {
        self.large.get_extended_float(index)
    }

    #[inline]
    pub fn get_small_int(&self, index: usize) -> u64 {
        self.small_int[index]
    }
}

// CACHED EXTENDED POWERS

/// Cached powers as a trait for a floating-point type.
pub(crate) trait ModeratePathCache {
    /// Get cached powers.
    fn get_powers() -> &'static ModeratePathPowers;
}

impl ModeratePathCache for ExtendedFloat {
    #[inline]
    fn get_powers() -> &'static ModeratePathPowers {
        cached_float80::get_powers()
    }
}
