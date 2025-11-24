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
use crate::double_double::DoubleDouble;
use crate::gamma::digamma_coeffs::digamma_coeefs;
use crate::logs::fast_log_d_to_dd;
use crate::tangent::cotpi_core;

#[inline]
fn approx_digamma_hard(x: f64) -> DoubleDouble {
    if x <= 12. {
        let x2 = DoubleDouble::from_exact_mult(x, x);
        let x4 = DoubleDouble::quick_mult(x2, x2);
        let x8 = DoubleDouble::quick_mult(x4, x4);
        let (p, q) = digamma_coeefs(x);

        let e0 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(p[1]),
            x,
            DoubleDouble::from_bit_pair(p[0]),
        );
        let e1 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(p[3]),
            x,
            DoubleDouble::from_bit_pair(p[2]),
        );
        let e2 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(p[5]),
            x,
            DoubleDouble::from_bit_pair(p[4]),
        );
        let e3 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(p[7]),
            x,
            DoubleDouble::from_bit_pair(p[6]),
        );
        let e4 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(p[9]),
            x,
            DoubleDouble::from_bit_pair(p[8]),
        );
        let e5 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(p[11]),
            x,
            DoubleDouble::from_bit_pair(p[10]),
        );

        let f0 = DoubleDouble::mul_add(x2, e1, e0);
        let f1 = DoubleDouble::mul_add(x2, e3, e2);
        let f2 = DoubleDouble::mul_add(x2, e5, e4);

        let g0 = DoubleDouble::mul_add(x4, f1, f0);

        let p_num = DoubleDouble::mul_add(x8, f2, g0);

        let rcp = DoubleDouble::from_quick_recip(x);

        let e0 = DoubleDouble::mul_f64_add_f64(
            DoubleDouble::from_bit_pair(q[1]),
            x,
            f64::from_bits(0x3ff0000000000000),
        );
        let e1 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(q[3]),
            x,
            DoubleDouble::from_bit_pair(q[2]),
        );
        let e2 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(q[5]),
            x,
            DoubleDouble::from_bit_pair(q[4]),
        );
        let e3 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(q[7]),
            x,
            DoubleDouble::from_bit_pair(q[6]),
        );
        let e4 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(q[9]),
            x,
            DoubleDouble::from_bit_pair(q[8]),
        );
        let e5 = DoubleDouble::mul_f64_add(
            DoubleDouble::from_bit_pair(q[11]),
            x,
            DoubleDouble::from_bit_pair(q[10]),
        );

        let f0 = DoubleDouble::mul_add(x2, e1, e0);
        let f1 = DoubleDouble::mul_add(x2, e3, e2);
        let f2 = DoubleDouble::mul_add(x2, e5, e4);

        let g0 = DoubleDouble::mul_add(x4, f1, f0);

        let p_den = DoubleDouble::mul_add(x8, f2, g0);

        let q = DoubleDouble::div(p_num, p_den);
        let r = DoubleDouble::quick_dd_sub(q, rcp);
        return r;
    }
    // digamma asymptotic expansion
    // digamma(x) ~ ln(z)+1/(2z)-sum_(n=1)^(infty)(Bernoulli(2n))/(2nz^(2n))
    // Generated in SageMath:
    // var('x')
    //
    // def bernoulli_terms(x, N):
    //     S = 0
    //     S += QQ(1)/QQ(2)/x
    //     for k in range(1, N+1):
    //         B = bernoulli(2*k)
    //         term = (B / QQ(2*k))*x**(-2*k)
    //         S += term
    //     return S
    //
    // terms = bernoulli_terms(x, 9)
    //
    // coeffs = [RealField(150)(terms.coefficient(x, n)) for n in range(0, terms.degree(x)+1, 1)]
    // for k in range(1, 13):
    //     c = terms.coefficient(x, -k)  # coefficient of x^(-k)
    //     if c == 0:
    //         continue
    //     print("f64::from_bits(" + double_to_hex(c) + "),")
    const C: [(u64, u64); 10] = [
        (0x3c55555555555555, 0x3fb5555555555555),
        (0xbc01111111111111, 0xbf81111111111111),
        (0x3c10410410410410, 0x3f70410410410410),
        (0xbbf1111111111111, 0xbf71111111111111),
        (0xbc0f07c1f07c1f08, 0x3f7f07c1f07c1f08),
        (0x3c39a99a99a99a9a, 0xbf95995995995996),
        (0x3c55555555555555, 0x3fb5555555555555),
        (0xbc77979797979798, 0xbfdc5e5e5e5e5e5e),
        (0xbc69180646019180, 0x40086e7f9b9fe6e8),
        (0x3ccad759ad759ad7, 0xc03a74ca514ca515),
    ];

    let rcp = DoubleDouble::from_quick_recip(x);
    let rcp_sqr = DoubleDouble::quick_mult(rcp, rcp);

    let rx = rcp_sqr;
    let x2 = DoubleDouble::quick_mult(rx, rx);
    let x4 = DoubleDouble::quick_mult(x2, x2);
    let x8 = DoubleDouble::quick_mult(x4, x4);

    let q0 = DoubleDouble::quick_mul_add(
        DoubleDouble::from_bit_pair(C[1]),
        rx,
        DoubleDouble::from_bit_pair(C[0]),
    );
    let q1 = DoubleDouble::quick_mul_add(
        DoubleDouble::from_bit_pair(C[3]),
        rx,
        DoubleDouble::from_bit_pair(C[2]),
    );
    let q2 = DoubleDouble::quick_mul_add(
        DoubleDouble::from_bit_pair(C[5]),
        rx,
        DoubleDouble::from_bit_pair(C[4]),
    );
    let q3 = DoubleDouble::quick_mul_add(
        DoubleDouble::from_bit_pair(C[7]),
        rx,
        DoubleDouble::from_bit_pair(C[6]),
    );
    let q4 = DoubleDouble::quick_mul_add(
        DoubleDouble::from_bit_pair(C[9]),
        rx,
        DoubleDouble::from_bit_pair(C[8]),
    );

    let q0 = DoubleDouble::quick_mul_add(x2, q1, q0);
    let q1 = DoubleDouble::quick_mul_add(x2, q3, q2);

    let r0 = DoubleDouble::quick_mul_add(x4, q1, q0);
    let mut p = DoubleDouble::quick_mul_add(x8, q4, r0);
    p = DoubleDouble::quick_mul_f64_add(
        rcp,
        f64::from_bits(0x3fe0000000000000),
        DoubleDouble::quick_mult(p, rcp_sqr),
    );

    let v_log = fast_log_d_to_dd(x);
    DoubleDouble::quick_dd_sub(v_log, p)
}

#[inline]
fn approx_digamma_hard_dd(x: DoubleDouble) -> DoubleDouble {
    if x.hi <= 12. {
        let x2 = DoubleDouble::quick_mult(x, x);
        let x4 = DoubleDouble::quick_mult(x2, x2);
        let x8 = DoubleDouble::quick_mult(x4, x4);
        let (p, q) = digamma_coeefs(x.hi);

        let e0 = DoubleDouble::mul_add(
            DoubleDouble::from_bit_pair(p[1]),
            x,
            DoubleDouble::from_bit_pair(p[0]),
        );
        let e1 = DoubleDouble::mul_add(
            DoubleDouble::from_bit_pair(p[3]),
            x,
            DoubleDouble::from_bit_pair(p[2]),
        );
        let e2 = DoubleDouble::mul_add(
            DoubleDouble::from_bit_pair(p[5]),
            x,
            DoubleDouble::from_bit_pair(p[4]),
        );
        let e3 = DoubleDouble::mul_add(
            DoubleDouble::from_bit_pair(p[7]),
            x,
            DoubleDouble::from_bit_pair(p[6]),
        );
        let e4 = DoubleDouble::mul_add(
            DoubleDouble::from_bit_pair(p[9]),
            x,
            DoubleDouble::from_bit_pair(p[8]),
        );
        let e5 = DoubleDouble::mul_add(
            DoubleDouble::from_bit_pair(p[11]),
            x,
            DoubleDouble::from_bit_pair(p[10]),
        );

        let f0 = DoubleDouble::mul_add(x2, e1, e0);
        let f1 = DoubleDouble::mul_add(x2, e3, e2);
        let f2 = DoubleDouble::mul_add(x2, e5, e4);

        let g0 = DoubleDouble::mul_add(x4, f1, f0);

        let p_num = DoubleDouble::mul_add(x8, f2, g0);

        let rcp = x.recip();

        let e0 = DoubleDouble::mul_add_f64(
            DoubleDouble::from_bit_pair(q[1]),
            x,
            f64::from_bits(0x3ff0000000000000),
        );
        let e1 = DoubleDouble::mul_add(
            DoubleDouble::from_bit_pair(q[3]),
            x,
            DoubleDouble::from_bit_pair(q[2]),
        );
        let e2 = DoubleDouble::mul_add(
            DoubleDouble::from_bit_pair(q[5]),
            x,
            DoubleDouble::from_bit_pair(q[4]),
        );
        let e3 = DoubleDouble::mul_add(
            DoubleDouble::from_bit_pair(q[7]),
            x,
            DoubleDouble::from_bit_pair(q[6]),
        );
        let e4 = DoubleDouble::mul_add(
            DoubleDouble::from_bit_pair(q[9]),
            x,
            DoubleDouble::from_bit_pair(q[8]),
        );
        let e5 = DoubleDouble::mul_add(
            DoubleDouble::from_bit_pair(q[11]),
            x,
            DoubleDouble::from_bit_pair(q[10]),
        );

        let f0 = DoubleDouble::mul_add(x2, e1, e0);
        let f1 = DoubleDouble::mul_add(x2, e3, e2);
        let f2 = DoubleDouble::mul_add(x2, e5, e4);

        let g0 = DoubleDouble::mul_add(x4, f1, f0);

        let p_den = DoubleDouble::mul_add(x8, f2, g0);

        let q = DoubleDouble::div(p_num, p_den);
        let r = DoubleDouble::quick_dd_sub(q, rcp);
        return r;
    }
    // digamma asymptotic expansion
    // digamma(x) ~ ln(z)+1/(2z)-sum_(n=1)^(infty)(Bernoulli(2n))/(2nz^(2n))
    // Generated in SageMath:
    // var('x')
    //
    // def bernoulli_terms(x, N):
    //     S = 0
    //     S += QQ(1)/QQ(2)/x
    //     for k in range(1, N+1):
    //         B = bernoulli(2*k)
    //         term = (B / QQ(2*k))*x**(-2*k)
    //         S += term
    //     return S
    //
    // terms = bernoulli_terms(x, 9)
    //
    // coeffs = [RealField(150)(terms.coefficient(x, n)) for n in range(0, terms.degree(x)+1, 1)]
    // for k in range(1, 13):
    //     c = terms.coefficient(x, -k)  # coefficient of x^(-k)
    //     if c == 0:
    //         continue
    //     print("f64::from_bits(" + double_to_hex(c) + "),")
    const C: [(u64, u64); 10] = [
        (0x3c55555555555555, 0x3fb5555555555555),
        (0xbc01111111111111, 0xbf81111111111111),
        (0x3c10410410410410, 0x3f70410410410410),
        (0xbbf1111111111111, 0xbf71111111111111),
        (0xbc0f07c1f07c1f08, 0x3f7f07c1f07c1f08),
        (0x3c39a99a99a99a9a, 0xbf95995995995996),
        (0x3c55555555555555, 0x3fb5555555555555),
        (0xbc77979797979798, 0xbfdc5e5e5e5e5e5e),
        (0xbc69180646019180, 0x40086e7f9b9fe6e8),
        (0x3ccad759ad759ad7, 0xc03a74ca514ca515),
    ];

    let rcp = x.recip();
    let rcp_sqr = DoubleDouble::quick_mult(rcp, rcp);

    let rx = rcp_sqr;
    let x2 = DoubleDouble::quick_mult(rx, rx);
    let x4 = DoubleDouble::quick_mult(x2, x2);
    let x8 = DoubleDouble::quick_mult(x4, x4);

    let q0 = DoubleDouble::quick_mul_add(
        DoubleDouble::from_bit_pair(C[1]),
        rx,
        DoubleDouble::from_bit_pair(C[0]),
    );
    let q1 = DoubleDouble::quick_mul_add(
        DoubleDouble::from_bit_pair(C[3]),
        rx,
        DoubleDouble::from_bit_pair(C[2]),
    );
    let q2 = DoubleDouble::quick_mul_add(
        DoubleDouble::from_bit_pair(C[5]),
        rx,
        DoubleDouble::from_bit_pair(C[4]),
    );
    let q3 = DoubleDouble::quick_mul_add(
        DoubleDouble::from_bit_pair(C[7]),
        rx,
        DoubleDouble::from_bit_pair(C[6]),
    );
    let q4 = DoubleDouble::quick_mul_add(
        DoubleDouble::from_bit_pair(C[9]),
        rx,
        DoubleDouble::from_bit_pair(C[8]),
    );

    let q0 = DoubleDouble::quick_mul_add(x2, q1, q0);
    let q1 = DoubleDouble::quick_mul_add(x2, q3, q2);

    let r0 = DoubleDouble::quick_mul_add(x4, q1, q0);
    let mut p = DoubleDouble::quick_mul_add(x8, q4, r0);
    p = DoubleDouble::quick_mul_f64_add(
        rcp,
        f64::from_bits(0x3fe0000000000000),
        DoubleDouble::quick_mult(p, rcp_sqr),
    );

    let mut v_log = fast_log_d_to_dd(x.hi);
    v_log.lo += x.lo / x.hi;
    DoubleDouble::quick_dd_sub(v_log, p)
}

/// Computes digamma(x)
///
pub fn f_digamma(x: f64) -> f64 {
    let xb = x.to_bits();
    if !x.is_normal() {
        if x.is_infinite() {
            return if x.is_sign_negative() {
                f64::NAN
            } else {
                f64::INFINITY
            };
        }
        if x.is_nan() {
            return f64::NAN;
        }
        if xb == 0 {
            return 1. / x;
        }
    }

    let dx = x;

    if x.abs() <= f64::EPSILON {
        // |x| < ulp(1)
        // digamma near where x -> 1 ~ Digamma[x] = -euler + O(x)
        // considering identity Digamma[x+1] = Digamma[x] + 1/x,
        // hence x < ulp(1) then x+1 ~= 1 that gives
        // Digamma[x] = Digamma[x+1] - 1/x = -euler - 1/x
        // euler is dropped since 1/x >> euler
        // that gives:
        // Digamma[x] = Digamma[x+1] - 1/x = -1/x
        return -1. / x;
    }

    if x < 0. {
        // at negative integers it's inf
        let k = x.floor();
        if k == x {
            return f64::NAN;
        }

        // reflection Gamma(1-x) + Gamma(x) = Pi/tan(PI*x)
        const PI: DoubleDouble =
            DoubleDouble::from_bit_pair((0x3ca1a62633145c07, 0x400921fb54442d18));
        let r = DoubleDouble::from_full_exact_sub(1., x);
        let mut result = PI * cotpi_core(-x);
        let app = approx_digamma_hard_dd(r);
        result = DoubleDouble::quick_dd_add(result, app);
        result.to_f64()
    } else {
        let app = approx_digamma_hard(dx);
        app.to_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_digamma() {
        assert_eq!(f_digamma(0.0019531200000040207), -512.5753182109892);
        assert_eq!(f_digamma(-13.999000000012591), -997.3224450000563);
        assert_eq!(f_digamma(13.999000000453323), 2.602844047257257);
    }
}
