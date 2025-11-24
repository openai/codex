/*
 * // Copyright (c) Radzivon Bartoshyk 6/2025. All rights reserved.
 * //
 * // Redistribution and use in source and binary forms, with or without modification,
 * // are permitted provided that the following conditions are met:
 * //
 * // 1.  Redistributions of source code must retain the above copyright notice, this
 * // list of conditions and the following disclaimer.
 * //
 * // 2.  Redistributions in binary form must reproduce the above copyright notice,
 * // this list of conditions and the following disclaimer in the documentation
 * // and/or other materials provided with the distribution.
 * //
 * // 3.  Neither the name of the copyright holder nor the names of its
 * // contributors may be used to endorse or promote products derived from
 * // this software without specific prior written permission.
 * //
 * // THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * // AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * // IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * // DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 * // FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * // DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * // SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 * // CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 * // OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * // OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
use crate::bits::{get_exponent_f32, get_exponent_f64};

#[inline]
pub const fn truncf(x: f32) -> f32 {
    // If x is infinity or NaN, return it.
    // If it is zero also we should return it as is, but the logic
    // later in this function takes care of it. But not doing a zero
    // check, we improve the run time of non-zero values.
    if x.is_infinite() || x.is_nan() {
        return x;
    }
    const FRACTION_LENGTH: u32 = 23;
    let exponent = get_exponent_f32(x);

    // If the exponent is greater than the most negative mantissa
    // exponent, then x is already an integer.
    if exponent >= FRACTION_LENGTH as i32 {
        return x;
    }

    // If the exponent is such that abs(x) is less than 1, then return 0.
    if exponent <= -1 {
        return if x.is_sign_negative() { -0.0 } else { 0.0 };
    }
    const FRACTION_MASK: u32 = (1 << FRACTION_LENGTH) - 1;
    let trim_size = FRACTION_LENGTH as i32 - exponent;
    let trunc_mantissa =
        ((x.to_bits() & FRACTION_MASK) >> trim_size).wrapping_shl(trim_size as u32);

    let prepared_bits = x.to_bits() & 0xFF800000;
    f32::from_bits(prepared_bits | trunc_mantissa)
}

#[inline]
pub const fn trunc(x: f64) -> f64 {
    // If x is infinity or NaN, return it.
    // If it is zero also we should return it as is, but the logic
    // later in this function takes care of it. But not doing a zero
    // check, we improve the run time of non-zero values.
    if x.is_infinite() || x.is_nan() {
        return x;
    }
    const FRACTION_LENGTH: u32 = 52;
    let exponent = get_exponent_f64(x);

    // If the exponent is greater than the most negative mantissa
    // exponent, then x is already an integer.
    if exponent >= FRACTION_LENGTH as i64 {
        return x;
    }

    // If the exponent is such that abs(x) is less than 1, then return 0.
    if exponent <= -1 {
        return if x.is_sign_negative() { -0.0 } else { 0.0 };
    }
    const FRACTION_MASK: u64 = (1 << FRACTION_LENGTH) - 1;
    let trim_size = FRACTION_LENGTH as i64 - exponent;
    let trunc_mantissa =
        ((x.to_bits() & FRACTION_MASK) >> trim_size).wrapping_shl(trim_size as u32);

    let prepared_bits = x.to_bits() & 0xFFF0000000000000;
    f64::from_bits(prepared_bits | trunc_mantissa)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncf() {
        assert_eq!(truncf(-1.0), -1.0);
        assert_eq!(truncf(1.0), 1.0);
        assert_eq!(truncf(1.234211), 1.0);
        assert_eq!(truncf(-1.234211), -1.0);
    }

    #[test]
    fn test_trunc() {
        assert_eq!(trunc(-1.0), -1.0);
        assert_eq!(trunc(1.0), 1.0);
        assert_eq!(trunc(1.234211), 1.0);
        assert_eq!(trunc(-1.234211), -1.0);
    }
}
