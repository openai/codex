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
use crate::double_double::DoubleDouble;
use crate::exponents::auxiliary::fast_ldexp;
use crate::exponents::exp::{EXP_REDUCE_T0, EXP_REDUCE_T1, to_denormal};
use std::hint::black_box;

#[inline]
fn exp10_poly_dd(z: DoubleDouble) -> DoubleDouble {
    const C: [(u64, u64); 6] = [
        (0xbcaf48ad494ea102, 0x40026bb1bbb55516),
        (0xbcae2bfab318d399, 0x40053524c73cea69),
        (0x3ca81f50779e162b, 0x4000470591de2ca4),
        (0x3c931a5cc5d3d313, 0x3ff2bd7609fd98c4),
        (0x3c8910de8c68a0c2, 0x3fe1429ffd336aa3),
        (0xbc605e703d496537, 0x3fca7ed7086882b4),
    ];

    let mut r = DoubleDouble::quick_mul_add(
        DoubleDouble::from_bit_pair(C[5]),
        z,
        DoubleDouble::from_bit_pair(C[4]),
    );
    r = DoubleDouble::quick_mul_add(r, z, DoubleDouble::from_bit_pair(C[3]));
    r = DoubleDouble::quick_mul_add(r, z, DoubleDouble::from_bit_pair(C[2]));
    r = DoubleDouble::quick_mul_add(r, z, DoubleDouble::from_bit_pair(C[1]));
    DoubleDouble::quick_mul_add(r, z, DoubleDouble::from_bit_pair(C[0]))
}

#[cold]
fn as_exp10_accurate(x: f64) -> f64 {
    let mut ix = x.to_bits();
    let t = (f64::from_bits(0x40ca934f0979a371) * x).round_ties_even();
    let jt: i64 = t as i64;
    let i1 = jt & 0x3f;
    let i0 = (jt >> 6) & 0x3f;
    let ie = jt >> 12;
    let t0 = DoubleDouble::from_bit_pair(EXP_REDUCE_T0[i0 as usize]);
    let t1 = DoubleDouble::from_bit_pair(EXP_REDUCE_T1[i1 as usize]);
    let dt = DoubleDouble::quick_mult(t0, t1);

    const L0: f64 = f64::from_bits(0x3f13441350800000);
    const L1: f64 = f64::from_bits(0xbd1f79fef311f12b);
    const L2: f64 = f64::from_bits(0xb9aac0b7c917826b);

    let dx = x - L0 * t;
    let dx_dd = DoubleDouble::quick_mult_f64(DoubleDouble::new(L2, L1), t);
    let dz = DoubleDouble::full_add_f64(dx_dd, dx);
    let mut f = exp10_poly_dd(dz);
    f = DoubleDouble::quick_mult(dz, f);

    let mut zfh: f64;

    if ix < 0xc0733a7146f72a42u64 {
        if (jt & 0xfff) == 0 {
            f = DoubleDouble::from_exact_add(f.hi, f.lo);
            let zt = DoubleDouble::from_exact_add(dt.hi, f.hi);
            f.hi = zt.lo;
            f = DoubleDouble::from_exact_add(f.hi, f.lo);
            ix = f.hi.to_bits();
            if (ix.wrapping_shl(12)) == 0 {
                let l = f.lo.to_bits();
                let sfh: i64 = ((ix as i64) >> 63) ^ ((l as i64) >> 63);
                ix = ix.wrapping_add(((1i64 << 51) ^ sfh) as u64);
            }
            zfh = zt.hi + f64::from_bits(ix);
        } else {
            f = DoubleDouble::quick_mult(f, dt);
            f = DoubleDouble::add(dt, f);
            f = DoubleDouble::from_exact_add(f.hi, f.lo);
            zfh = f.hi;
        }
        zfh = fast_ldexp(zfh, ie as i32);
    } else {
        ix = (1u64.wrapping_sub(ie as u64)) << 52;
        f = DoubleDouble::quick_mult(f, dt);
        f = DoubleDouble::add(dt, f);

        let zt = DoubleDouble::from_exact_add(f64::from_bits(ix), f.hi);
        f.hi = zt.hi;
        f.lo += zt.lo;

        zfh = to_denormal(f.to_f64());
    }
    zfh
}

/// Computes exp10
///
/// Max found ULP 0.5
pub fn f_exp10(x: f64) -> f64 {
    let mut ix = x.to_bits();
    let aix = ix & 0x7fff_ffff_ffff_ffff;
    if aix > 0x40734413509f79feu64 {
        // |x| > 0x40734413509f79fe
        if aix > 0x7ff0000000000000u64 {
            return x + x;
        } // nan
        if aix == 0x7ff0000000000000u64 {
            return if (ix >> 63) != 0 { 0.0 } else { x };
        }
        if (ix >> 63) == 0 {
            return f64::from_bits(0x7fe0000000000000) * 2.0; // x > 308.255
        }
        if aix > 0x407439b746e36b52u64 {
            // x < -323.607
            return black_box(f64::from_bits(0x0018000000000000))
                * black_box(f64::from_bits(0x3c80000000000000));
        }
    }

    // check x integer to avoid a spurious inexact exception
    if ix.wrapping_shl(16) == 0 && (aix >> 48) <= 0x4036 {
        let kx = x.round_ties_even();
        if kx == x {
            let k = kx as i64;
            if k >= 0 {
                let mut r = 1.0;
                for _ in 0..k {
                    r *= 10.0;
                }
                return r;
            }
        }
    }
    /* avoid spurious underflow: for |x| <= 2.41082e-17
    exp10(x) rounds to 1 to nearest */
    if aix <= 0x3c7bcb7b1526e50eu64 {
        return 1.0 + x; // |x| <= 2.41082e-17
    }
    let t = (f64::from_bits(0x40ca934f0979a371) * x).round_ties_even();
    let jt = t as i64;
    let i1 = jt & 0x3f;
    let i0 = (jt >> 6) & 0x3f;
    let ie = jt >> 12;
    let t00 = EXP_REDUCE_T0[i0 as usize];
    let t01 = EXP_REDUCE_T1[i1 as usize];
    let t0 = DoubleDouble::from_bit_pair(t00);
    let t1 = DoubleDouble::from_bit_pair(t01);
    let mut tz = DoubleDouble::quick_mult(t0, t1);
    const L0: f64 = f64::from_bits(0x3f13441350800000);
    const L1: f64 = f64::from_bits(0x3d1f79fef311f12b);
    let dx = f_fmla(-L1, t, f_fmla(-L0, t, x));
    let dx2 = dx * dx;

    const CH: [u64; 4] = [
        0x40026bb1bbb55516,
        0x40053524c73cea69,
        0x4000470591fd74e1,
        0x3ff2bd760a1f32a5,
    ];

    let p0 = f_fmla(dx, f64::from_bits(CH[1]), f64::from_bits(CH[0]));
    let p1 = f_fmla(dx, f64::from_bits(CH[3]), f64::from_bits(CH[2]));

    let p = f_fmla(dx2, p1, p0);

    let mut fh = tz.hi;
    let fx = tz.hi * dx;
    let mut fl = f_fmla(fx, p, tz.lo);
    const EPS: f64 = 1.63e-19;
    if ix < 0xc0733a7146f72a42u64 {
        // x > -307.653
        // x > -0x1.33a7146f72a42p+8
        let ub = fh + (fl + EPS);
        let lb = fh + (fl - EPS);

        if lb != ub {
            return as_exp10_accurate(x);
        }
        fh = fast_ldexp(fh + fl, ie as i32);
    } else {
        // x <= -307.653: exp10(x) < 2^-1022
        ix = 1u64.wrapping_sub(ie as u64).wrapping_shl(52);
        tz = DoubleDouble::from_exact_add(f64::from_bits(ix), fh);
        fl += tz.lo;

        let ub = fh + (fl + EPS);
        let lb = fh + (fl - EPS);

        if lb != ub {
            return as_exp10_accurate(x);
        }

        fh = to_denormal(fh + fl);
    }
    fh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exp10f() {
        assert_eq!(f_exp10(-3.370739843267434), 0.00042585343701025656);
        assert_eq!(f_exp10(1.), 10.0);
        assert_eq!(f_exp10(2.), 100.0);
        assert_eq!(f_exp10(3.), 1000.0);
        assert_eq!(f_exp10(4.), 10000.0);
        assert_eq!(f_exp10(5.), 100000.0);
        assert_eq!(f_exp10(6.), 1000000.0);
        assert_eq!(f_exp10(7.), 10000000.0);
        assert_eq!(f_exp10(f64::INFINITY), f64::INFINITY);
        assert_eq!(f_exp10(f64::NEG_INFINITY), 0.);
        assert!(f_exp10(f64::NAN).is_nan());
    }
}
