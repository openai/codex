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
use crate::acospi::INV_PI_DD;
use crate::asin::asin_eval;
use crate::asin_eval_dyadic::asin_eval_dyadic;
use crate::common::{dd_fmla, dyad_fmla, f_fmla};
use crate::double_double::DoubleDouble;
use crate::dyadic_float::{DyadicFloat128, DyadicSign};

/// Computes asin(x)/PI
///
/// Max found ULP 0.5
pub fn f_asinpi(x: f64) -> f64 {
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

            if x_e < E_BIAS - 56 {
                if (x_abs.to_bits().wrapping_shl(12)) == 0x59af9a1194efe000u64 {
                    let e = (x.to_bits() >> 52) & 0x7ff;
                    let h = f64::from_bits(0x3c7b824198b94a89);
                    let l = f64::from_bits(0x391fffffffffffff);
                    let mut t = (if x > 0. { 1.0f64 } else { -1.0f64 }).to_bits();
                    t = t.wrapping_sub(0x3c9u64.wrapping_sub(e).wrapping_shl(52));
                    return f_fmla(l, f64::from_bits(t), h * f64::from_bits(t));
                }

                let h = x * INV_PI_DD.hi;
                let sx = x * f64::from_bits(0x4690000000000000); /* scale x */
                let mut l = dd_fmla(sx, INV_PI_DD.hi, -h * f64::from_bits(0x4690000000000000));
                l = dd_fmla(sx, INV_PI_DD.lo, l);
                /* scale back */
                let res = dyad_fmla(l, f64::from_bits(0x3950000000000000), h);
                return res;
            }

            /* We use the Sollya polynomial 0x1.45f306dc9c882a53f84eafa3ea4p-2 * x
            + 0x1.b2995e7b7b606p-5 * x^3, with relative error bounded by 2^-106.965
            on [2^-53, 2^-26] */
            const C1H: f64 = f64::from_bits(0x3fd45f306dc9c883);
            const C1L: f64 = f64::from_bits(0xbc76b01ec5417057);
            const C3: f64 = f64::from_bits(0x3fab2995e7b7b606);
            let h = C1H;
            let l = dd_fmla(C3, x * x, C1L);
            /* multiply h+l by x */
            let hh = h * x;
            let mut ll = dd_fmla(h, x, -hh);
            /* hh+ll = h*x */
            ll = dd_fmla(l, x, ll);
            return hh + ll;
        }

        let x_sq = DoubleDouble::from_exact_mult(x, x);
        let err = x_abs * f64::from_bits(0x3cc0000000000000);
        // Polynomial approximation:
        //   p ~ asin(x)/x

        let (p, err) = asin_eval(x_sq, err);
        // asin(x) ~ x * (ASIN_COEFFS[idx][0] + p)
        let mut r0 = DoubleDouble::from_exact_mult(x, p.hi);
        let mut r_lo = f_fmla(x, p.lo, r0.lo);

        r0 = DoubleDouble::mult(DoubleDouble::new(r_lo, r0.hi), INV_PI_DD);
        r_lo = r0.lo;

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
        let mut r = x_f128.quick_mul(&p_f128);
        r = r.quick_mul(&crate::acospi::INV_PI_F128);
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
            return x * 0.5; // asinpi_specific
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
    let mut r = DoubleDouble::from_exact_add(PI_OVER_TWO.hi, -r0.hi);

    let mut r_lo = PI_OVER_TWO.lo - r0.lo + r.lo;

    let p = DoubleDouble::mult(DoubleDouble::new(r_lo, r.hi), INV_PI_DD);
    r_lo = p.lo;
    r.hi = p.hi;

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
    let r0_f128 = m_v * p_f128;
    let mut r_f128 = PI_OVER_TWO_F128 + r0_f128;

    if x.is_sign_negative() {
        r_f128.sign = DyadicSign::Neg;
    }

    r_f128 = r_f128.quick_mul(&crate::acospi::INV_PI_F128);

    r_f128.fast_as_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_asinpi_test() {
        assert_eq!(
            f_asinpi(-0.00000000032681723993732703),
            -0.00000000010402915844735117
        );
        assert_eq!(f_asinpi(0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000017801371778309684), 0.00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000005666352624669099);
        assert_eq!(f_asinpi(0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000026752519513526076), 0.000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008515591441480124);
        assert_eq!(f_asinpi(-0.4), -0.13098988043445461);
        assert_eq!(f_asinpi(-0.8), -0.2951672353008666);
        assert_eq!(f_asinpi(0.4332432142124432), 0.14263088583055605);
        assert_eq!(f_asinpi(0.8543543534343434), 0.326047108714517);
        assert_eq!(f_asinpi(0.00323146509843243), 0.0010286090778797426);
    }
}
