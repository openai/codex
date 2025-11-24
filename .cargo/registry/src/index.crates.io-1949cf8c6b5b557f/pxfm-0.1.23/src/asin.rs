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
use crate::asin_eval_dyadic::asin_eval_dyadic;
use crate::common::f_fmla;
use crate::double_double::DoubleDouble;
use crate::dyadic_float::{DyadicFloat128, DyadicSign};

static ASIN_COEFFS: [[u64; 12]; 9] = [
    [
        0x3ff0000000000000,
        0x0000000000000000,
        0x3fc5555555555555,
        0x3c65555555555555,
        0x3fb3333333333333,
        0x3fa6db6db6db6db7,
        0x3f9f1c71c71c71c7,
        0x3f96e8ba2e8ba2e9,
        0x3f91c4ec4ec4ec4f,
        0x3f8c99999999999a,
        0x3f87a87878787878,
        0x3f83fde50d79435e,
    ],
    [
        0x3ff015a397cf0f1c,
        0xbc8eebd6ccfe3ee3,
        0x3fc5f3581be7b08b,
        0xbc65df80d0e7237d,
        0x3fb4519ddf1ae530,
        0x3fa8eb4b6eeb1696,
        0x3fa17bc85420fec8,
        0x3f9a8e39b5dcad81,
        0x3f953f8df127539b,
        0x3f91a485a0b0130a,
        0x3f8e20e6e4930020,
        0x3f8a466a7030f4c9,
    ],
    [
        0x3ff02be9ce0b87cd,
        0x3c7e5d09da2e0f04,
        0x3fc69ab5325bc359,
        0xbc692f480cfede2d,
        0x3fb58a4c3097aab1,
        0x3fab3db36068dd80,
        0x3fa3b94821846250,
        0x3f9eedc823765d21,
        0x3f998e35d756be6b,
        0x3f95ea4f1b32731a,
        0x3f9355115764148e,
        0x3f916a5853847c91,
    ],
    [
        0x3ff042dc6a65ffbf,
        0xbc8c7ea28dce95d1,
        0x3fc74c4bd7412f9d,
        0x3c5447024c0a3c87,
        0x3fb6e09c6d2b72b9,
        0x3faddd9dcdae5315,
        0x3fa656f1f64058b8,
        0x3fa21a42e4437101,
        0x3f9eed0350b7edb2,
        0x3f9b6bc877e58c52,
        0x3f9903a0872eb2a4,
        0x3f974da839ddd6d8,
    ],
    [
        0x3ff05a8621feb16b,
        0xbc7e5b33b1407c5f,
        0x3fc809186c2e57dd,
        0xbc33dcb4d6069407,
        0x3fb8587d99442dc5,
        0x3fb06c23d1e75be3,
        0x3fa969024051c67d,
        0x3fa54e4f934aacfd,
        0x3fa2d60a732dbc9c,
        0x3fa149f0c046eac7,
        0x3fa053a56dba1fba,
        0x3f9f7face3343992,
    ],
    [
        0x3ff072f2b6f1e601,
        0xbc92dcbb05419970,
        0x3fc8d2397127aeba,
        0x3c6ead0c497955fb,
        0x3fb9f68df88da518,
        0x3fb21ee26a5900d7,
        0x3fad08e7081b53a9,
        0x3fa938dd661713f7,
        0x3fa71b9f299b72e6,
        0x3fa5fbc7d2450527,
        0x3fa58573247ec325,
        0x3fa585a174a6a4ce,
    ],
    [
        0x3ff08c2f1d638e4c,
        0x3c7b47c159534a3d,
        0x3fc9a8f592078624,
        0xbc6ea339145b65cd,
        0x3fbbc04165b57aab,
        0x3fb410df5f58441d,
        0x3fb0ab6bdf5f8f70,
        0x3fae0b92eea1fce1,
        0x3fac9094e443a971,
        0x3fac34651d64bc74,
        0x3facaa008d1af080,
        0x3fadc165bc0c4fc5,
    ],
    [
        0x3ff0a649a73e61f2,
        0x3c874ac0d817e9c7,
        0x3fca8ec30dc93890,
        0xbc48ab1c0eef300c,
        0x3fbdbc11ea95061b,
        0x3fb64e371d661328,
        0x3fb33e0023b3d895,
        0x3fb2042269c243ce,
        0x3fb1cce74bda2230,
        0x3fb244d425572ce9,
        0x3fb34d475c7f1e3e,
        0x3fb4d4e653082ad3,
    ],
    [
        0x3ff0c152382d7366,
        0xbc9ee6913347c2a6,
        0x3fcb8550d62bfb6d,
        0xbc6d10aec3f116d5,
        0x3fbff1bde0fa3ca0,
        0x3fb8e5f3ab69f6a4,
        0x3fb656be8b6527ce,
        0x3fb5c39755dc041a,
        0x3fb661e6ebd40599,
        0x3fb7ea3dddee2a4f,
        0x3fba4f439abb4869,
        0x3fbd9181c0fda658,
    ],
];

#[inline]
pub(crate) fn asin_eval(u: DoubleDouble, err: f64) -> (DoubleDouble, f64) {
    // k = round(u * 32).
    let k = (u.hi * f64::from_bits(0x4040000000000000)).round();
    let idx = k as u64;
    // y = u - k/32.
    let y_hi = f_fmla(k, f64::from_bits(0xbfa0000000000000), u.hi); // Exact
    let y = DoubleDouble::from_exact_add(y_hi, u.lo);
    let y2 = y.hi * y.hi;
    // Add double-double errors in addition to the relative errors from y2.
    let err = f_fmla(err, y2, f64::from_bits(0x3990000000000000));
    let coeffs = ASIN_COEFFS[idx as usize];
    let c0 = DoubleDouble::quick_mult(
        y,
        DoubleDouble::new(f64::from_bits(coeffs[3]), f64::from_bits(coeffs[2])),
    );
    let c1 = f_fmla(y.hi, f64::from_bits(coeffs[5]), f64::from_bits(coeffs[4]));
    let c2 = f_fmla(y.hi, f64::from_bits(coeffs[7]), f64::from_bits(coeffs[6]));
    let c3 = f_fmla(y.hi, f64::from_bits(coeffs[9]), f64::from_bits(coeffs[8]));
    let c4 = f_fmla(y.hi, f64::from_bits(coeffs[11]), f64::from_bits(coeffs[10]));

    let y4 = y2 * y2;
    let d0 = f_fmla(y2, c2, c1);
    let d1 = f_fmla(y2, c4, c3);

    let mut r = DoubleDouble::from_exact_add(f64::from_bits(coeffs[0]), c0.hi);

    let e1 = f_fmla(y4, d1, d0);

    r.lo = f_fmla(y2, e1, f64::from_bits(coeffs[1]) + c0.lo + r.lo);

    (r, err)
}

/// Computes asin(x)
///
/// Max found ULP 0.5
pub fn f_asin(x: f64) -> f64 {
    let x_e = (x.to_bits() >> 52) & 0x7ff;
    const E_BIAS: u64 = (1u64 << (11 - 1u64)) - 1u64;

    let x_abs = f64::from_bits(x.to_bits() & 0x7fff_ffff_ffff_ffff);

    // |x| < 0.5.
    if x_e < E_BIAS - 1 {
        // |x| < 2^-26.
        if x_e < E_BIAS - 26 {
            // When |x| < 2^-26, the relative error of the approximation asin(x) ~ x
            // is:
            //   |asin(x) - x| / |asin(x)| < |x^3| / (6|x|)
            //                             = x^2 / 6
            //                             < 2^-54
            //                             < epsilon(1)/2.
            //   = x otherwise. ,
            if x.abs() == 0. {
                return x;
            }
            // Get sign(x) * min_normal.
            let eps = f64::copysign(f64::MIN_POSITIVE, x);
            let normalize_const = if x_e == 0 { eps } else { 0.0 };
            let scaled_normal =
                f_fmla(x + normalize_const, f64::from_bits(0x4350000000000000), eps);
            return f_fmla(
                scaled_normal,
                f64::from_bits(0x3c90000000000000),
                -normalize_const,
            );
        }

        let x_sq = DoubleDouble::from_exact_mult(x, x);
        let err = x_abs * f64::from_bits(0x3cc0000000000000);
        // Polynomial approximation:
        //   p ~ asin(x)/x

        let (p, err) = asin_eval(x_sq, err);
        // asin(x) ~ x * (ASIN_COEFFS[idx][0] + p)
        let r0 = DoubleDouble::from_exact_mult(x, p.hi);
        let r_lo = f_fmla(x, p.lo, r0.lo);

        let r_upper = r0.hi + (r_lo + err);
        let r_lower = r0.hi + (r_lo - err);

        if r_upper == r_lower {
            return r_upper;
        }

        // Ziv's accuracy test failed, perform 128-bit calculation.

        // Recalculate mod 1/64.
        let idx = (x_sq.hi * f64::from_bits(0x4050000000000000)).round() as usize;

        // Get x^2 - idx/64 exactly.  When FMA is available, double-double
        // multiplication will be correct for all rounding modes. Otherwise, we use
        // Float128 directly.
        let x_f128 = DyadicFloat128::new_from_f64(x);

        let u: DyadicFloat128;
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            // u = x^2 - idx/64
            let u_hi = DyadicFloat128::new_from_f64(f_fmla(
                idx as f64,
                f64::from_bits(0xbf90000000000000),
                x_sq.hi,
            ));
            u = u_hi.quick_add(&DyadicFloat128::new_from_f64(x_sq.lo));
        }

        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let x_sq_f128 = x_f128.quick_mul(&x_f128);
            u = x_sq_f128.quick_add(&DyadicFloat128::new_from_f64(
                idx as f64 * (f64::from_bits(0xbf90000000000000)),
            ));
        }

        let p_f128 = asin_eval_dyadic(u, idx);
        let r = x_f128.quick_mul(&p_f128);
        return r.fast_as_f64();
    }

    const PI_OVER_TWO: DoubleDouble = DoubleDouble::new(
        f64::from_bits(0x3c91a62633145c07),
        f64::from_bits(0x3ff921fb54442d18),
    );

    let x_sign = if x.is_sign_negative() { -1.0 } else { 1.0 };

    // |x| >= 1
    if x_e >= E_BIAS {
        // x = +-1, asin(x) = +- pi/2
        if x_abs == 1.0 {
            // return +- pi/2
            return f_fmla(x_sign, PI_OVER_TWO.hi, x_sign * PI_OVER_TWO.lo);
        }
        // |x| > 1, return NaN.
        if x.is_nan() {
            return x;
        }
        return f64::NAN;
    }

    // u = (1 - |x|)/2
    let u = f_fmla(x_abs, -0.5, 0.5);
    // v_hi + v_lo ~ sqrt(u).
    // Let:
    //   h = u - v_hi^2 = (sqrt(u) - v_hi) * (sqrt(u) + v_hi)
    // Then:
    //   sqrt(u) = v_hi + h / (sqrt(u) + v_hi)
    //           ~ v_hi + h / (2 * v_hi)
    // So we can use:
    //   v_lo = h / (2 * v_hi).
    // Then,
    //   asin(x) ~ pi/2 - 2*(v_hi + v_lo) * P(u)
    let v_hi = u.sqrt();
    let h;
    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        h = f_fmla(v_hi, -v_hi, u);
    }
    #[cfg(not(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        let v_hi_sq = DoubleDouble::from_exact_mult(v_hi, v_hi);
        h = (u - v_hi_sq.hi) - v_hi_sq.lo;
    }
    // Scale v_lo and v_hi by 2 from the formula:
    //   vh = v_hi * 2
    //   vl = 2*v_lo = h / v_hi.
    let vh = v_hi * 2.0;
    let vl = h / v_hi;

    // Polynomial approximation:
    //   p ~ asin(sqrt(u))/sqrt(u)
    let err = vh * f64::from_bits(0x3cc0000000000000);

    let (p, err) = asin_eval(DoubleDouble::new(0.0, u), err);

    // Perform computations in double-double arithmetic:
    //   asin(x) = pi/2 - (v_hi + v_lo) * (ASIN_COEFFS[idx][0] + p)
    let r0 = DoubleDouble::quick_mult(DoubleDouble::new(vl, vh), p);
    let r = DoubleDouble::from_exact_add(PI_OVER_TWO.hi, -r0.hi);

    let r_lo = PI_OVER_TWO.lo - r0.lo + r.lo;

    let (r_upper, r_lower);

    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        r_upper = f_fmla(r.hi, x_sign, f_fmla(r_lo, x_sign, err));
        r_lower = f_fmla(r.hi, x_sign, f_fmla(r_lo, x_sign, -err));
    }
    #[cfg(not(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        let r_lo = r_lo * x_sign;
        let r_hi = r.hi * x_sign;
        r_upper = r_hi + (r_lo + err);
        r_lower = r.hi + (r_lo - err);
    }

    if r_upper == r_lower {
        return r_upper;
    }

    // Ziv's accuracy test failed, we redo the computations in Float128.
    // Recalculate mod 1/64.
    let idx = (u * f64::from_bits(0x4050000000000000)).round() as usize;

    // After the first step of Newton-Raphson approximating v = sqrt(u), we have
    // that:
    //   sqrt(u) = v_hi + h / (sqrt(u) + v_hi)
    //      v_lo = h / (2 * v_hi)
    // With error:
    //   sqrt(u) - (v_hi + v_lo) = h * ( 1/(sqrt(u) + v_hi) - 1/(2*v_hi) )
    //                           = -h^2 / (2*v * (sqrt(u) + v)^2).
    // Since:
    //   (sqrt(u) + v_hi)^2 ~ (2sqrt(u))^2 = 4u,
    // we can add another correction term to (v_hi + v_lo) that is:
    //   v_ll = -h^2 / (2*v_hi * 4u)
    //        = -v_lo * (h / 4u)
    //        = -vl * (h / 8u),
    // making the errors:
    //   sqrt(u) - (v_hi + v_lo + v_ll) = O(h^3)
    // well beyond 128-bit precision needed.

    // Get the rounding error of vl = 2 * v_lo ~ h / vh
    // Get full product of vh * vl
    let vl_lo;
    #[cfg(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        vl_lo = f_fmla(-v_hi, vl, h) / v_hi;
    }
    #[cfg(not(any(
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "fma"
        ),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        let vh_vl = DoubleDouble::from_exact_mult(v_hi, vl);
        vl_lo = ((h - vh_vl.hi) - vh_vl.lo) / v_hi;
    }

    // vll = 2*v_ll = -vl * (h / (4u)).
    let t = h * (-0.25) / u;
    let vll = f_fmla(vl, t, vl_lo);
    // m_v = -(v_hi + v_lo + v_ll).
    let mv0 = DyadicFloat128::new_from_f64(vl) + DyadicFloat128::new_from_f64(vll);
    let mut m_v = DyadicFloat128::new_from_f64(vh) + mv0;
    m_v.sign = DyadicSign::Neg;

    // Perform computations in Float128:
    //   asin(x) = pi/2 - (v_hi + v_lo + vll) * P(u).
    let y_f128 =
        DyadicFloat128::new_from_f64(f_fmla(idx as f64, f64::from_bits(0xbf90000000000000), u));

    const PI_OVER_TWO_F128: DyadicFloat128 = DyadicFloat128 {
        sign: DyadicSign::Pos,
        exponent: -127,
        mantissa: 0xc90fdaa2_2168c234_c4c6628b_80dc1cd1_u128,
    };

    let p_f128 = asin_eval_dyadic(y_f128, idx);
    let r0_f128 = m_v.quick_mul(&p_f128);
    let mut r_f128 = PI_OVER_TWO_F128.quick_add(&r0_f128);

    if x.is_sign_negative() {
        r_f128.sign = DyadicSign::Neg;
    }

    r_f128.fast_as_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_asin_test() {
        assert_eq!(f_asin(-0.4), -0.41151684606748806);
        assert_eq!(f_asin(-0.8), -0.9272952180016123);
        assert_eq!(f_asin(0.3), 0.3046926540153975);
        assert_eq!(f_asin(0.6), 0.6435011087932844);
    }
}
