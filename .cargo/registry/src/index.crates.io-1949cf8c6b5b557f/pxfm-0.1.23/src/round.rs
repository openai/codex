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
use crate::bits::{get_exponent_f32, get_exponent_f64, mantissa_f32, mantissa_f64};

#[inline]
pub const fn roundf(x: f32) -> f32 {
    // If x is infinity NaN or zero, return it.
    if !x.is_normal() {
        return x;
    }

    let exponent = get_exponent_f32(x);

    const FRACTION_LENGTH: u32 = 23;

    // If the exponent is greater than the most negative mantissa
    // exponent, then x is already an integer.
    if exponent >= FRACTION_LENGTH as i32 {
        return x;
    }

    if exponent == -1 {
        // Absolute value of x is greater than equal to 0.5 but less than 1.
        return if x.is_sign_negative() { -1.0 } else { 1.0 };
    }

    if exponent <= -2 {
        // Absolute value of x is less than 0.5.
        return if x.is_sign_negative() { -0.0 } else { 0.0 };
    }

    let trim_size = (FRACTION_LENGTH as i32).wrapping_sub(exponent);
    let half_bit_set = mantissa_f32(x) & (1u32 << (trim_size - 1)) != 0;
    let x_u = x.to_bits();
    let trunc_u: u32 = (x_u >> trim_size).wrapping_shl(trim_size as u32);

    // If x is already an integer, return it.
    if trunc_u == x_u {
        return x;
    }

    let trunc_value = f32::from_bits(trunc_u);

    if !half_bit_set {
        // Franctional part is less than 0.5 so round value is the
        // same as the trunc value.
        trunc_value
    } else if x.is_sign_negative() {
        trunc_value - 1.0
    } else {
        trunc_value + 1.0
    }
}

#[inline]
pub const fn round(x: f64) -> f64 {
    // If x is infinity NaN or zero, return it.
    if !x.is_normal() {
        return x;
    }

    let exponent = get_exponent_f64(x);

    const FRACTION_LENGTH: u64 = 52;

    // If the exponent is greater than the most negative mantissa
    // exponent, then x is already an integer.
    if exponent >= FRACTION_LENGTH as i64 {
        return x;
    }

    if exponent == -1 {
        // Absolute value of x is greater than equal to 0.5 but less than 1.
        return if x.is_sign_negative() { -1.0 } else { 1.0 };
    }

    if exponent <= -2 {
        // Absolute value of x is less than 0.5.
        return if x.is_sign_negative() { -0.0 } else { 0.0 };
    }

    let trim_size = (FRACTION_LENGTH as i64).wrapping_sub(exponent);
    let half_bit_set = mantissa_f64(x) & (1u64 << (trim_size.wrapping_sub(1))) != 0;
    let x_u = x.to_bits();
    let trunc_u: u64 = (x_u >> trim_size).wrapping_shl(trim_size as u32);

    // If x is already an integer, return it.
    if trunc_u == x_u {
        return x;
    }

    let trunc_value = f64::from_bits(trunc_u);

    if !half_bit_set {
        // Franctional part is less than 0.5 so round value is the
        // same as the trunc value.
        trunc_value
    } else if x.is_sign_negative() {
        trunc_value - 1.0
    } else {
        trunc_value + 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundf() {
        assert_eq!(roundf(0f32), 0.0f32.round());
        assert_eq!(roundf(1f32), 1.0f32.round());
        assert_eq!(roundf(1.2f32), 1.2f32.round());
        assert_eq!(roundf(-1.2f32), (-1.2f32).round());
        assert_eq!(roundf(-1.6f32), (-1.6f32).round());
        assert_eq!(roundf(-1.5f32), (-1.5f32).round());
        assert_eq!(roundf(1.6f32), 1.6f32.round());
        assert_eq!(roundf(1.5f32), 1.5f32.round());
        assert_eq!(roundf(2.5f32), 2.5f32.round());
    }

    #[test]
    fn test_round() {
        assert_eq!(round(0.), 0.0f64.round());
        assert_eq!(round(1.), 1.0f64.round());
        assert_eq!(round(1.2), 1.2f64.round());
        assert_eq!(round(-1.2), (-1.2f64).round());
        assert_eq!(round(-1.6), (-1.6f64).round());
        assert_eq!(round(-1.5), (-1.5f64).round());
        assert_eq!(round(1.6), 1.6f64.round());
        assert_eq!(round(1.5), 1.5f64.round());
        assert_eq!(round(2.5), 2.5f64.round());
    }
}
