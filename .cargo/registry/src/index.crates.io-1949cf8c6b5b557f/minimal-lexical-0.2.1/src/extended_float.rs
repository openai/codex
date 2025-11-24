// FLOAT TYPE

#![doc(hidden)]

use crate::num::Float;

/// Extended precision floating-point type.
///
/// Private implementation, exposed only for testing purposes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExtendedFloat {
    /// Mantissa for the extended-precision float.
    pub mant: u64,
    /// Binary exponent for the extended-precision float.
    pub exp: i32,
}

/// Converts an `ExtendedFloat` to the closest machine float type.
#[inline(always)]
pub fn extended_to_float<F: Float>(x: ExtendedFloat) -> F {
    let mut word = x.mant;
    word |= (x.exp as u64) << F::MANTISSA_SIZE;
    F::from_bits(word)
}
