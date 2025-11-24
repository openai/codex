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
use crate::logs::fast_log_dd;
use crate::polyeval::{f_polyeval6, f_polyeval7, f_polyeval8};
use crate::pow_exec::exp_dd_fast;
use crate::sincospi::f_fast_sinpi_dd;

/// Computes gamma(x)
///
/// ulp 1
pub fn f_tgamma(x: f64) -> f64 {
    let x_a = f64::from_bits(x.to_bits() & 0x7fff_ffff_ffff_ffff);

    if !x.is_normal() {
        if x == 0.0 {
            return 1. / x;
        }

        if x.is_nan() {
            return x + x;
        }

        if x.is_infinite() {
            if x.is_sign_negative() {
                return f64::NAN;
            }
            return x;
        }
    }

    if x >= 171.624 {
        return f64::INFINITY;
    }

    if x.floor() == x {
        if x < 0. {
            return f64::NAN;
        }
        if x < 38. {
            let mut t = DoubleDouble::new(0., 1.);
            let k = x as i64;
            let mut x0 = 1.0;
            for _i in 1..k {
                t = DoubleDouble::quick_mult_f64(t, x0);
                t = DoubleDouble::from_exact_add(t.hi, t.lo);
                x0 += 1.0;
            }
            return t.to_f64();
        }
    }

    if x <= -184.0 {
        /* negative non-integer */
        /* For x <= -184, x non-integer, |gamma(x)| < 2^-1078.  */
        static SIGN: [f64; 2] = [
            f64::from_bits(0x0010000000000000),
            f64::from_bits(0x8010000000000000),
        ];
        let k = x as i64;
        return f64::from_bits(0x0010000000000000) * SIGN[((k & 1) != 0) as usize];
    }

    const EULER_DD: DoubleDouble =
        DoubleDouble::from_bit_pair((0xbc56cb90701fbfab, 0x3fe2788cfc6fb619));

    if x_a < 0.006 {
        if x_a.to_bits() < (0x71e0000000000000u64 >> 1) {
            // |x| < 0x1p-112
            return 1. / x;
        }
        if x_a < 2e-10 {
            // x is tiny then Gamma(x) = 1/x - euler
            let p = DoubleDouble::full_dd_sub(DoubleDouble::from_quick_recip(x), EULER_DD);
            return p.to_f64();
        } else if x_a < 2e-6 {
            // x is small then Gamma(x) = 1/x - euler + a2*x
            // a2 = 1/12 * (6 * euler^2 + pi^2)
            const A2: DoubleDouble =
                DoubleDouble::from_bit_pair((0x3c8dd92b465a8221, 0x3fefa658c23b1578));
            let rcp = DoubleDouble::from_quick_recip(x);
            let p = DoubleDouble::full_dd_add(DoubleDouble::mul_f64_add(A2, x, -EULER_DD), rcp);
            return p.to_f64();
        }

        // Laurent series of Gamma(x)
        const C: [(u64, u64); 8] = [
            (0x3c8dd92b465a8221, 0x3fefa658c23b1578),
            (0x3c53a4f483760950, 0xbfed0a118f324b63),
            (0x3c7fabe4f7369157, 0x3fef6a51055096b5),
            (0x3c8c9fc795fc6142, 0xbfef6c80ec38b67b),
            (0xbc5042339d62e721, 0x3fefc7e0a6eb310b),
            (0xbc86fd0d8bdc0c1e, 0xbfefdf3f157b7a39),
            (0xbc89b912df09395d, 0x3feff07b5a17ff6c),
            (0x3c4e626faf780ff9, 0xbfeff803d68a0bd4),
        ];
        let rcp = DoubleDouble::from_quick_recip(x);
        let mut p = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(C[7]),
            x,
            DoubleDouble::from_bit_pair(C[6]),
        );
        p = DoubleDouble::mul_f64_add(p, x, DoubleDouble::from_bit_pair(C[5]));
        p = DoubleDouble::mul_f64_add(p, x, DoubleDouble::from_bit_pair(C[4]));
        p = DoubleDouble::mul_f64_add(p, x, DoubleDouble::from_bit_pair(C[3]));
        p = DoubleDouble::mul_f64_add(p, x, DoubleDouble::from_bit_pair(C[2]));
        p = DoubleDouble::mul_f64_add(p, x, DoubleDouble::from_bit_pair(C[1]));
        p = DoubleDouble::mul_f64_add(p, x, DoubleDouble::from_bit_pair(C[0]));
        let z = DoubleDouble::mul_f64_add(p, x, DoubleDouble::full_dd_sub(rcp, EULER_DD));
        return z.to_f64();
    }

    let mut fact = DoubleDouble::new(0., 0.0f64);
    let mut parity = 1.0;
    const PI: DoubleDouble = DoubleDouble::from_bit_pair((0x3ca1a62633145c07, 0x400921fb54442d18));
    let mut dy = DoubleDouble::new(0., x);
    let mut result: DoubleDouble;

    // reflection
    if dy.hi < 0. {
        if dy.hi.floor() == dy.hi {
            return f64::NAN;
        }
        dy.hi = f64::from_bits(dy.hi.to_bits() & 0x7fff_ffff_ffff_ffff);
        let y1 = x_a.floor();
        let fraction = x_a - y1;
        if fraction != 0.0
        // is it an integer?
        {
            // is it odd or even?
            if y1 != (y1 * 0.5).trunc() * 2.0 {
                parity = -1.0;
            }
            fact = DoubleDouble::div(-PI, f_fast_sinpi_dd(fraction));
            fact = DoubleDouble::from_exact_add(fact.hi, fact.lo);
            dy = DoubleDouble::from_full_exact_add(dy.hi, 1.0);
        }
    }

    if dy.hi < 12.0 {
        let y1 = dy;
        let z: DoubleDouble;
        let mut n = 0;
        // x is in (0.06, 1.0).
        if dy.hi < 1.0 {
            z = dy;
            dy = DoubleDouble::full_add_f64(dy, 1.0);
        } else
        // x is in [1.0, max].
        {
            n = dy.hi as i32 - 1;
            dy = DoubleDouble::full_add_f64(dy, -n as f64);
            z = DoubleDouble::full_add_f64(dy, -1.0);
        }

        // Gamma(x+1) on [1;2] generated by Wolfram Mathematica:
        // <<FunctionApproximations`
        // ClearAll["Global`*"]
        // f[x_]:=Gamma[x+1]
        // {err0, approx}=MiniMaxApproximation[f[z],{z,{0,1 },9,8},WorkingPrecision->90]
        // num=Numerator[approx][[1]];
        // den=Denominator[approx][[1]];
        // poly=den;
        // coeffs=CoefficientList[poly,z];
        // TableForm[Table[Row[{"'",NumberForm[coeffs[[i+1]],{50,50}, ExponentFunction->(Null&)],"',"}],{i,0,Length[coeffs]-1}]]
        let ps_num = f_polyeval8(
            z.hi,
            f64::from_bits(0x3fb38b80568a42aa),
            f64::from_bits(0xbf8e7685b00d63a6),
            f64::from_bits(0xbf80629ed2c48f1a),
            f64::from_bits(0xbf6dfc4cdbcee96a),
            f64::from_bits(0xbf471816939dc42b),
            f64::from_bits(0xbf24bede7d8b3c20),
            f64::from_bits(0xbeef56936d891e42),
            f64::from_bits(0xbec3e2b405350813),
        );
        let mut p_num = DoubleDouble::mul_f64_add(
            z,
            ps_num,
            DoubleDouble::from_bit_pair((0xbc700f441aea2edb, 0x3fd218bdde7878b8)),
        );
        p_num = DoubleDouble::mul_add(
            z,
            p_num,
            DoubleDouble::from_bit_pair((0x3b7056a45a2fa50e, 0x3ff0000000000000)),
        );
        p_num = DoubleDouble::from_exact_add(p_num.hi, p_num.lo);

        let ps_den = f_polyeval7(
            z.hi,
            f64::from_bits(0xbfdaa4f09f0caab1),
            f64::from_bits(0xbfc960ba48423f9d),
            f64::from_bits(0x3fb6873b64e8ccd6),
            f64::from_bits(0x3f69ea1ca5b8a225),
            f64::from_bits(0xbf77b166f68a2e63),
            f64::from_bits(0x3f4fd1eff9193728),
            f64::from_bits(0xbf0c1a43f4985c97),
        );
        let mut p_den = DoubleDouble::mul_f64_add(
            z,
            ps_den,
            DoubleDouble::from_bit_pair((0xbc759594c51ad8b7, 0x3feb84ebebabf275)),
        );
        p_den = DoubleDouble::mul_add_f64(z, p_den, f64::from_bits(0x3ff0000000000000));
        p_den = DoubleDouble::from_exact_add(p_den.hi, p_den.lo);
        result = DoubleDouble::div(p_num, p_den);
        if y1.hi < dy.hi {
            result = DoubleDouble::div(result, y1);
        } else if y1.hi > dy.hi {
            for _ in 0..n {
                result = DoubleDouble::mult(result, dy);
                dy = DoubleDouble::full_add_f64(dy, 1.0);
            }
        }
    } else {
        if x > 171.624e+0 {
            return f64::INFINITY;
        }
        // Stirling's approximation of Log(Gamma) and then Exp[Log[Gamma]]
        let y_recip = dy.recip();
        let y_sqr = DoubleDouble::mult(y_recip, y_recip);
        // Bernoulli coefficients generated by SageMath:
        // var('x')
        // def bernoulli_terms(x, N):
        //     S = 0
        //     for k in range(1, N+1):
        //         B = bernoulli(2*k)
        //         term = (B / (2*k*(2*k-1))) * x**((2*k-1))
        //         S += term
        //     return S
        //
        // terms = bernoulli_terms(x, 7)
        let bernoulli_poly_s = f_polyeval6(
            y_sqr.hi,
            f64::from_bits(0xbf66c16c16c16c17),
            f64::from_bits(0x3f4a01a01a01a01a),
            f64::from_bits(0xbf43813813813814),
            f64::from_bits(0x3f4b951e2b18ff23),
            f64::from_bits(0xbf5f6ab0d9993c7d),
            f64::from_bits(0x3f7a41a41a41a41a),
        );
        let bernoulli_poly = DoubleDouble::mul_f64_add(
            y_sqr,
            bernoulli_poly_s,
            DoubleDouble::from_bit_pair((0x3c55555555555555, 0x3fb5555555555555)),
        );
        // Log[Gamma(x)] = x*log(x) - x + 1/2*Log(2*PI/x) + bernoulli_terms
        const LOG2_PI_OVER_2: DoubleDouble =
            DoubleDouble::from_bit_pair((0xbc865b5a1b7ff5df, 0x3fed67f1c864beb5));
        let mut log_gamma = DoubleDouble::add(
            DoubleDouble::mul_add(bernoulli_poly, y_recip, -dy),
            LOG2_PI_OVER_2,
        );
        let dy_log = fast_log_dd(dy);
        log_gamma = DoubleDouble::mul_add(
            DoubleDouble::from_exact_add(dy_log.hi, dy_log.lo),
            DoubleDouble::add_f64(dy, -0.5),
            log_gamma,
        );
        let log_prod = log_gamma.to_f64();
        if log_prod >= 690. {
            // underflow/overflow case
            log_gamma = DoubleDouble::quick_mult_f64(log_gamma, 0.5);
            result = exp_dd_fast(log_gamma);
            let exp_result = result;
            result.hi *= parity;
            result.lo *= parity;
            if fact.lo != 0. && fact.hi != 0. {
                // y / x = y / (z*z) = y / z * 1/z
                result = DoubleDouble::from_exact_add(result.hi, result.lo);
                result = DoubleDouble::div(fact, result);
                result = DoubleDouble::div(result, exp_result);
            } else {
                result = DoubleDouble::quick_mult(result, exp_result);
            }

            let err = f_fmla(
                result.hi.abs(),
                f64::from_bits(0x3c20000000000000), // 2^-61
                f64::from_bits(0x3bd0000000000000), // 2^-65
            );
            let ub = result.hi + (result.lo + err);
            let lb = result.hi + (result.lo - err);
            if ub == lb {
                return result.to_f64();
            }
            return result.to_f64();
        }
        result = exp_dd_fast(log_gamma);
    }

    if fact.lo != 0. && fact.hi != 0. {
        // y / x = y / (z*z) = y / z * 1/z
        result = DoubleDouble::from_exact_add(result.hi, result.lo);
        result = DoubleDouble::div(fact, result);
    }
    result.to_f64() * parity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tgamma() {
        // assert_eq!(f_tgamma(6.757812502211891), 459.54924419209556);
        assert_eq!(f_tgamma(-1.70000000042915), 2.513923520668069);
        assert_eq!(f_tgamma(5.), 24.);
        assert_eq!(f_tgamma(24.), 25852016738884980000000.);
        assert_eq!(f_tgamma(6.4324324), 255.1369211339094);
        assert_eq!(f_tgamma(f64::INFINITY), f64::INFINITY);
        assert_eq!(f_tgamma(0.), f64::INFINITY);
        assert!(f_tgamma(f64::NAN).is_nan());
    }
}
