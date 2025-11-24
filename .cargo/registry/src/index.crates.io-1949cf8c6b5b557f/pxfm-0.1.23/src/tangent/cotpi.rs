/*
 * // Copyright (c) Radzivon Bartoshyk 8/2025. All rights reserved.
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
use crate::double_double::DoubleDouble;
use crate::sincospi::reduce_pi_64;
use crate::sincospi_tables::SINPI_K_PI_OVER_64;
use crate::tangent::tanpi::tanpi_eval;

#[cold]
fn cotpi_hard(x: f64, msin_k: DoubleDouble, cos_k: DoubleDouble) -> DoubleDouble {
    const C: [(u64, u64); 6] = [
        (0x3ca1a62632712fc8, 0x400921fb54442d18),
        (0xbcc052338fbb4528, 0x4024abbce625be53),
        (0x3ced42454c5f85b3, 0x404466bc6775aad9),
        (0xbd00c7d6a971a560, 0x40645fff9b4b244d),
        (0x3d205970eff53274, 0x40845f46e96c3a0b),
        (0xbd3589489ad24fc4, 0x40a4630551cd123d),
    ];
    let x2 = DoubleDouble::from_exact_mult(x, x);
    let mut tan_y = DoubleDouble::quick_mul_add(
        x2,
        DoubleDouble::from_bit_pair(C[5]),
        DoubleDouble::from_bit_pair(C[4]),
    );
    tan_y = DoubleDouble::quick_mul_add(x2, tan_y, DoubleDouble::from_bit_pair(C[3]));
    tan_y = DoubleDouble::quick_mul_add(x2, tan_y, DoubleDouble::from_bit_pair(C[2]));
    tan_y = DoubleDouble::quick_mul_add(x2, tan_y, DoubleDouble::from_bit_pair(C[1]));
    tan_y = DoubleDouble::quick_mul_add(x2, tan_y, DoubleDouble::from_bit_pair(C[0]));
    tan_y = DoubleDouble::quick_mult_f64(tan_y, x);

    // num = sin(k*pi) + tan(y*pi) * cos(k*pi)
    let num = DoubleDouble::mul_add(tan_y, cos_k, -msin_k);
    // den = cos(k*pi) - tan(y*pi) * sin(k*pi)
    let den = DoubleDouble::mul_add(tan_y, msin_k, cos_k);
    // cot = den / num
    DoubleDouble::div(den, num)
}

/// Computes cotangent 1/tan(PI*x)
///
/// ulp 0.5
pub fn f_cotpi(x: f64) -> f64 {
    if x == 0. {
        return if x.is_sign_negative() {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let ax = x.to_bits() & 0x7fff_ffff_ffff_ffff;
    if ax >= (0x7ffu64 << 52) {
        // NaN, Inf
        if ax > (0x7ffu64 << 52) {
            return x + x;
        } // NaN
        return f64::NAN; // x=Inf
    }
    let e: i32 = (ax >> 52) as i32 - 1023;
    if e > 0 {
        if e >= 52 {
            // when |x| > 2^53 it's always an integer
            return f64::copysign(f64::INFINITY, x);
        }
        // |x| > 1 and |x| < 2^53
        let m = (ax & ((1u64 << 52) - 1)) | (1u64 << 52); // mantissa with hidden 1
        let shift = 52 - e;

        let frac = m & ((1u64 << shift) - 1);
        if frac == (1u64 << (shift - 1)) {
            // |x| is always integer.5 means it's inf
            return f64::copysign(0., x);
        }
    }

    if ax <= 0x3cb0000000000000 {
        // for tiny x ( |x| < f64::EPSILON ) just small taylor expansion
        // cot(PI*x) ~ 1/(PI*x) + O(x^3)
        const ONE_OVER_PI: DoubleDouble =
            DoubleDouble::from_bit_pair((0xbc76b01ec5417056, 0x3fd45f306dc9c883));
        if ax <= 0x3ca0000000000000 {
            // |x| <= 2^-53, renormalize value
            let e: i32 = (ax >> 52) as i32;
            let sc = f64::from_bits((2045i64 - e as i64).wrapping_shl(52) as u64);
            let dx = x * sc;
            let q0 = DoubleDouble::quick_mult(ONE_OVER_PI, DoubleDouble::from_quick_recip(dx));
            let r = q0.to_f64() * sc;
            return r;
        }
        let q0 = DoubleDouble::quick_mult(ONE_OVER_PI, DoubleDouble::from_quick_recip(x));
        let r = q0.to_f64();
        return r;
    }

    // argument reduction
    let (y, k) = reduce_pi_64(x);

    if y == 0.0 {
        let km = (k.abs() & 63) as i32; // k mod 64

        match km {
            0 => return f64::copysign(f64::INFINITY, x), // cotpi(n) = 0
            32 => return f64::copysign(0., x),           // cotpi(n+0.5) = ±∞
            16 => return f64::copysign(1.0, x),          // cotpi(n+0.25) = ±1
            48 => return -f64::copysign(1.0, x),         // cotpi(n+0.75) = ∓1
            _ => {}
        }
    }

    let msin_k = DoubleDouble::from_bit_pair(
        SINPI_K_PI_OVER_64[((k as u64).wrapping_add(64) & 127) as usize],
    );
    let cos_k = DoubleDouble::from_bit_pair(
        SINPI_K_PI_OVER_64[((k as u64).wrapping_add(32) & 127) as usize],
    );

    // tanpi_eval returns:
    // - rs.tan_y = tan(pi * y)          -> tangent of the remainder
    // - rs.msin_k = sin(pi * k)         -> sine of the main angle multiple
    // - rs.cos_k  = cos(pi * k)         -> cosine of the main angle multiple
    let tan_y = tanpi_eval(y);
    // num = sin(k*pi) + tan(y*pi) * cos(k*pi)
    let num = DoubleDouble::mul_add(tan_y, cos_k, -msin_k);
    // den = cos(k*pi) - tan(y*pi) * sin(k*pi)
    let den = DoubleDouble::mul_add(tan_y, msin_k, cos_k);
    // cot = den / num
    let tan_value = DoubleDouble::div(den, num);
    let err = f_fmla(
        tan_value.hi,
        f64::from_bits(0x3bf0000000000000), // 2^-64
        f64::from_bits(0x3b60000000000000), // 2^-73
    );
    let ub = tan_value.hi + (tan_value.lo + err);
    let lb = tan_value.hi + (tan_value.lo - err);
    if ub == lb {
        return tan_value.to_f64();
    }
    cotpi_hard(y, msin_k, cos_k).to_f64()
}

#[inline]
pub(crate) fn cotpi_core(x: f64) -> DoubleDouble {
    // argument reduction
    let (y, k) = reduce_pi_64(x);

    let msin_k = DoubleDouble::from_bit_pair(
        SINPI_K_PI_OVER_64[((k as u64).wrapping_add(64) & 127) as usize],
    );
    let cos_k = DoubleDouble::from_bit_pair(
        SINPI_K_PI_OVER_64[((k as u64).wrapping_add(32) & 127) as usize],
    );

    // tanpi_eval returns:
    // - rs.tan_y = tan(pi * y)          -> tangent of the remainder
    // - rs.msin_k = sin(pi * k)         -> sine of the main angle multiple
    // - rs.cos_k  = cos(pi * k)         -> cosine of the main angle multiple
    let tan_y = tanpi_eval(y);
    // num = sin(k*pi) + tan(y*pi) * cos(k*pi)
    let num = DoubleDouble::mul_add(tan_y, cos_k, -msin_k);
    // den = cos(k*pi) - tan(y*pi) * sin(k*pi)
    let den = DoubleDouble::mul_add(tan_y, msin_k, cos_k);
    // cot = den / num
    DoubleDouble::div(den, num)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cotpi() {
        assert_eq!(f_cotpi(3.382112265043898e-306), 9.411570676518013e304);
    }
}
