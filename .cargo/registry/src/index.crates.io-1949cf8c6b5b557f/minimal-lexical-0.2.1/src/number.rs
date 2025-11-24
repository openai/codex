//! Representation of a float as the significant digits and exponent.
//!
//! This is adapted from [fast-float-rust](https://github.com/aldanor/fast-float-rust),
//! a port of [fast_float](https://github.com/fastfloat/fast_float) to Rust.

#![doc(hidden)]

#[cfg(feature = "nightly")]
use crate::fpu::set_precision;
use crate::num::Float;

/// Representation of a number as the significant digits and exponent.
///
/// This is only used if the exponent base and the significant digit
/// radix are the same, since we need to be able to move powers in and
/// out of the exponent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Number {
    /// The exponent of the float, scaled to the mantissa.
    pub exponent: i32,
    /// The significant digits of the float.
    pub mantissa: u64,
    /// If the significant digits were truncated.
    pub many_digits: bool,
}

impl Number {
    /// Detect if the float can be accurately reconstructed from native floats.
    #[inline]
    pub fn is_fast_path<F: Float>(&self) -> bool {
        F::MIN_EXPONENT_FAST_PATH <= self.exponent
            && self.exponent <= F::MAX_EXPONENT_DISGUISED_FAST_PATH
            && self.mantissa <= F::MAX_MANTISSA_FAST_PATH
            && !self.many_digits
    }

    /// The fast path algorithmn using machine-sized integers and floats.
    ///
    /// This is extracted into a separate function so that it can be attempted before constructing
    /// a Decimal. This only works if both the mantissa and the exponent
    /// can be exactly represented as a machine float, since IEE-754 guarantees
    /// no rounding will occur.
    ///
    /// There is an exception: disguised fast-path cases, where we can shift
    /// powers-of-10 from the exponent to the significant digits.
    pub fn try_fast_path<F: Float>(&self) -> Option<F> {
        // The fast path crucially depends on arithmetic being rounded to the correct number of bits
        // without any intermediate rounding. On x86 (without SSE or SSE2) this requires the precision
        // of the x87 FPU stack to be changed so that it directly rounds to 64/32 bit.
        // The `set_precision` function takes care of setting the precision on architectures which
        // require setting it by changing the global state (like the control word of the x87 FPU).
        #[cfg(feature = "nightly")]
        let _cw = set_precision::<F>();

        if self.is_fast_path::<F>() {
            let max_exponent = F::MAX_EXPONENT_FAST_PATH;
            Some(if self.exponent <= max_exponent {
                // normal fast path
                let value = F::from_u64(self.mantissa);
                if self.exponent < 0 {
                    // SAFETY: safe, since the `exponent <= max_exponent`.
                    value / unsafe { F::pow_fast_path((-self.exponent) as _) }
                } else {
                    // SAFETY: safe, since the `exponent <= max_exponent`.
                    value * unsafe { F::pow_fast_path(self.exponent as _) }
                }
            } else {
                // disguised fast path
                let shift = self.exponent - max_exponent;
                // SAFETY: safe, since `shift <= (max_disguised - max_exponent)`.
                let int_power = unsafe { F::int_pow_fast_path(shift as usize, 10) };
                let mantissa = self.mantissa.checked_mul(int_power)?;
                if mantissa > F::MAX_MANTISSA_FAST_PATH {
                    return None;
                }
                // SAFETY: safe, since the `table.len() - 1 == max_exponent`.
                F::from_u64(mantissa) * unsafe { F::pow_fast_path(max_exponent as _) }
            })
        } else {
            None
        }
    }
}
