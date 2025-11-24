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
pub const fn ceilf(x: f32) -> f32 {
    // If x is infinity NaN or zero, return it.
    if !x.is_normal() {
        return x;
    }

    let is_neg = x.is_sign_negative();
    let exponent = get_exponent_f32(x);

    // If the exponent is greater than the most negative mantissa
    // exponent, then x is already an integer.
    const FRACTION_LENGTH: u32 = 23;
    if exponent >= FRACTION_LENGTH as i32 {
        return x;
    }

    if exponent <= -1 {
        return if is_neg { -0.0 } else { 1.0 };
    }

    let trim_size = (FRACTION_LENGTH as i32).wrapping_sub(exponent);
    let x_u = x.to_bits();
    let trunc_u = x_u
        .wrapping_shr(trim_size as u32)
        .wrapping_shl(trim_size as u32);

    // If x is already an integer, return it.
    if trunc_u == x_u {
        return x;
    }

    let trunc_value = f32::from_bits(trunc_u);

    // If x is negative, the ceil operation is equivalent to the trunc operation.
    if is_neg {
        return trunc_value;
    }

    trunc_value + 1.0
}

#[inline]
pub const fn ceil(x: f64) -> f64 {
    // If x is infinity NaN or zero, return it.
    if !x.is_normal() {
        return x;
    }

    let is_neg = x.is_sign_negative();
    let exponent = get_exponent_f64(x);

    // If the exponent is greater than the most negative mantissa
    // exponent, then x is already an integer.
    const FRACTION_LENGTH: u64 = 52;
    if exponent >= FRACTION_LENGTH as i64 {
        return x;
    }

    if exponent <= -1 {
        return if is_neg { -0.0 } else { 1.0 };
    }

    let trim_size = (FRACTION_LENGTH as i64).wrapping_sub(exponent);
    let x_u = x.to_bits();
    let trunc_u = x_u
        .wrapping_shr(trim_size as u32)
        .wrapping_shl(trim_size as u32);

    // If x is already an integer, return it.
    if trunc_u == x_u {
        return x;
    }

    let trunc_value = f64::from_bits(trunc_u);

    // If x is negative, the ceil operation is equivalent to the trunc operation.
    if is_neg {
        return trunc_value;
    }

    trunc_value + 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ceilf() {
        assert_eq!(ceilf(0.0), 0.0);
        assert_eq!(ceilf(10.0), 10.0);
        assert_eq!(ceilf(10.1), 11.0);
        assert_eq!(ceilf(-9.0), -9.0);
        assert_eq!(ceilf(-9.5), -9.0);
    }

    #[test]
    fn test_ceil() {
        assert_eq!(ceil(0.0), 0.0);
        assert_eq!(ceil(10.0), 10.0);
        assert_eq!(ceil(10.1), 11.0);
        assert_eq!(ceil(-9.0), -9.0);
        assert_eq!(ceil(-9.5), -9.0);
    }
}
