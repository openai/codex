// Adapted from https://github.com/Alexhuszagh/rust-lexical.

// FLOAT TYPE

use super::num::*;
use super::rounding::*;
use super::shift::*;

/// Extended precision floating-point type.
///
/// Private implementation, exposed only for testing purposes.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExtendedFloat {
    /// Mantissa for the extended-precision float.
    pub mant: u64,
    /// Binary exponent for the extended-precision float.
    pub exp: i32,
}

impl ExtendedFloat {
    // PROPERTIES

    // OPERATIONS

    /// Multiply two normalized extended-precision floats, as if by `a*b`.
    ///
    /// The precision is maximal when the numbers are normalized, however,
    /// decent precision will occur as long as both values have high bits
    /// set. The result is not normalized.
    ///
    /// Algorithm:
    ///     1. Non-signed multiplication of mantissas (requires 2x as many bits as input).
    ///     2. Normalization of the result (not done here).
    ///     3. Addition of exponents.
    pub(crate) fn mul(&self, b: &ExtendedFloat) -> ExtendedFloat {
        // Logic check, values must be decently normalized prior to multiplication.
        debug_assert!((self.mant & u64::HIMASK != 0) && (b.mant & u64::HIMASK != 0));

        // Extract high-and-low masks.
        let ah = self.mant >> u64::HALF;
        let al = self.mant & u64::LOMASK;
        let bh = b.mant >> u64::HALF;
        let bl = b.mant & u64::LOMASK;

        // Get our products
        let ah_bl = ah * bl;
        let al_bh = al * bh;
        let al_bl = al * bl;
        let ah_bh = ah * bh;

        let mut tmp = (ah_bl & u64::LOMASK) + (al_bh & u64::LOMASK) + (al_bl >> u64::HALF);
        // round up
        tmp += 1 << (u64::HALF - 1);

        ExtendedFloat {
            mant: ah_bh + (ah_bl >> u64::HALF) + (al_bh >> u64::HALF) + (tmp >> u64::HALF),
            exp: self.exp + b.exp + u64::FULL,
        }
    }

    /// Multiply in-place, as if by `a*b`.
    ///
    /// The result is not normalized.
    #[inline]
    pub(crate) fn imul(&mut self, b: &ExtendedFloat) {
        *self = self.mul(b);
    }

    // NORMALIZE

    /// Normalize float-point number.
    ///
    /// Shift the mantissa so the number of leading zeros is 0, or the value
    /// itself is 0.
    ///
    /// Get the number of bytes shifted.
    #[inline]
    pub(crate) fn normalize(&mut self) -> u32 {
        // Note:
        // Using the cltz intrinsic via leading_zeros is way faster (~10x)
        // than shifting 1-bit at a time, via while loop, and also way
        // faster (~2x) than an unrolled loop that checks at 32, 16, 4,
        // 2, and 1 bit.
        //
        // Using a modulus of pow2 (which will get optimized to a bitwise
        // and with 0x3F or faster) is slightly slower than an if/then,
        // however, removing the if/then will likely optimize more branched
        // code as it removes conditional logic.

        // Calculate the number of leading zeros, and then zero-out
        // any overflowing bits, to avoid shl overflow when self.mant == 0.
        let shift = if self.mant == 0 {
            0
        } else {
            self.mant.leading_zeros()
        };
        shl(self, shift as i32);
        shift
    }

    // ROUND

    /// Lossy round float-point number to native mantissa boundaries.
    #[inline]
    pub(crate) fn round_to_native<F, Algorithm>(&mut self, algorithm: Algorithm)
    where
        F: Float,
        Algorithm: FnOnce(&mut ExtendedFloat, i32),
    {
        round_to_native::<F, _>(self, algorithm);
    }

    // FROM

    /// Create extended float from native float.
    #[inline]
    pub fn from_float<F: Float>(f: F) -> ExtendedFloat {
        from_float(f)
    }

    // INTO

    /// Convert into default-rounded, lower-precision native float.
    #[inline]
    pub(crate) fn into_float<F: Float>(mut self) -> F {
        self.round_to_native::<F, _>(round_nearest_tie_even);
        into_float(self)
    }

    /// Convert into downward-rounded, lower-precision native float.
    #[inline]
    pub(crate) fn into_downward_float<F: Float>(mut self) -> F {
        self.round_to_native::<F, _>(round_downward);
        into_float(self)
    }
}

// FROM FLOAT

// Import ExtendedFloat from native float.
#[inline]
pub(crate) fn from_float<F>(f: F) -> ExtendedFloat
where
    F: Float,
{
    ExtendedFloat {
        mant: u64::as_cast(f.mantissa()),
        exp: f.exponent(),
    }
}

// INTO FLOAT

// Export extended-precision float to native float.
//
// The extended-precision float must be in native float representation,
// with overflow/underflow appropriately handled.
#[inline]
pub(crate) fn into_float<F>(fp: ExtendedFloat) -> F
where
    F: Float,
{
    // Export floating-point number.
    if fp.mant == 0 || fp.exp < F::DENORMAL_EXPONENT {
        // sub-denormal, underflow
        F::ZERO
    } else if fp.exp >= F::MAX_EXPONENT {
        // overflow
        F::from_bits(F::INFINITY_BITS)
    } else {
        // calculate the exp and fraction bits, and return a float from bits.
        let exp: u64;
        if (fp.exp == F::DENORMAL_EXPONENT) && (fp.mant & F::HIDDEN_BIT_MASK.as_u64()) == 0 {
            exp = 0;
        } else {
            exp = (fp.exp + F::EXPONENT_BIAS) as u64;
        }
        let exp = exp << F::MANTISSA_SIZE;
        let mant = fp.mant & F::MANTISSA_MASK.as_u64();
        F::from_bits(F::Unsigned::as_cast(mant | exp))
    }
}
