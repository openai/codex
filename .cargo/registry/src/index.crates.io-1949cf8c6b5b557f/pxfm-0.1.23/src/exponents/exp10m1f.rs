/*
 * // Copyright (c) Radzivon Bartoshyk 7/2025. All rights reserved.
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
use crate::common::f_fmla;
use crate::exponents::exp10f::EXP10F_COEFFS;
use crate::polyeval::f_polyeval3;

#[cold]
fn exp10m1f_small(x: f32) -> f32 {
    let dx = x as f64;
    let dx_sq = dx * dx;
    let c0 = dx * f64::from_bits(EXP10F_COEFFS[0]);
    let c1 = f_fmla(
        dx,
        f64::from_bits(EXP10F_COEFFS[2]),
        f64::from_bits(EXP10F_COEFFS[1]),
    );
    let c2 = f_fmla(
        dx,
        f64::from_bits(EXP10F_COEFFS[4]),
        f64::from_bits(EXP10F_COEFFS[3]),
    );
    // 10^dx - 1 ~ (1 + COEFFS[0] * dx + ... + COEFFS[4] * dx^5) - 1
    //           = COEFFS[0] * dx + ... + COEFFS[4] * dx^5
    f_polyeval3(dx_sq, c0, c1, c2) as f32
}

/// Computes 10^x - 1
///
/// Max ULP 0.5
#[inline]
pub fn f_exp10m1f(x: f32) -> f32 {
    let x_u = x.to_bits();
    let x_abs = x_u & 0x7fff_ffffu32;

    // When x >= log10(2^128), or x is nan
    if x.is_sign_positive() && x_u >= 0x421a_209bu32 {
        // x >= log10(2^128) and 10^x - 1 rounds to +inf, or x is +inf or nan
        return x + f32::INFINITY;
    }

    if x_abs <= 0x3b9a_209bu32 {
        // |x| <= 0.004703594
        return exp10m1f_small(x);
    }

    // When x <= log10(2^-25), or x is nan
    if x_u >= 0xc0f0d2f1 {
        // exp10m1(-inf) = -1
        if x.is_infinite() {
            return -1.0;
        }
        // exp10m1(nan) = nan
        if x.is_nan() {
            return x;
        }

        if x_u == 0xc0f0d2f1 {
            return f32::from_bits(0xbf7fffff); // -1.0f + 0x1.0p-24f
        }
        return -1.0;
    }

    // Exact outputs when x = 1, 2, ..., 10.
    // Quick check mask: 0x800f'ffffU = ~(bits of 1.0f | ... | bits of 10.0f)
    if x_u & 0x800f_ffffu32 == 0 {
        match x_u {
            0x3f800000u32 => return 9.0,             // x = 1.0f
            0x40000000u32 => return 99.0,            // x = 2.0f
            0x40400000u32 => return 999.0,           // x = 3.0f
            0x40800000u32 => return 9_999.0,         // x = 4.0f
            0x40a00000u32 => return 99_999.0,        // x = 5.0f
            0x40c00000u32 => return 999_999.0,       // x = 6.0f
            0x40e00000u32 => return 9_999_999.0,     // x = 7.0f
            0x41000000u32 => return 99_999_999.0,    // x = 8.0f
            0x41100000u32 => return 999_999_999.0,   // x = 9.0f
            0x41200000u32 => return 9_999_999_999.0, // x = 10.0f
            _ => {}
        }
    }

    // Range reduction: 10^x = 2^(mid + hi) * 10^lo
    //   rr = (2^(mid + hi), lo)
    let rr = crate::exponents::exp10f::exp_b_range_reduc(x);

    // The low part is approximated by a degree-5 minimax polynomial.
    // 10^lo ~ 1 + COEFFS[0] * lo + ... + COEFFS[4] * lo^5
    let lo_sq = rr.lo * rr.lo;
    let c0 = f_fmla(rr.lo, f64::from_bits(EXP10F_COEFFS[0]), 1.0);
    let c1 = f_fmla(
        rr.lo,
        f64::from_bits(EXP10F_COEFFS[2]),
        f64::from_bits(EXP10F_COEFFS[1]),
    );
    let c2 = f_fmla(
        rr.lo,
        f64::from_bits(EXP10F_COEFFS[4]),
        f64::from_bits(EXP10F_COEFFS[3]),
    );
    let exp10_lo = f_polyeval3(lo_sq, c0, c1, c2);
    // 10^x - 1 = 2^(mid + hi) * 10^lo - 1
    //          ~ mh * exp10_lo - 1
    f_fmla(exp10_lo, rr.hi, -1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exp10m1f() {
        assert_eq!(f_exp10m1f(0.0), 0.0);
        assert_eq!(f_exp10m1f(1.0), 9.0);
        assert_eq!(f_exp10m1f(1.5), 30.622776);
    }
}
