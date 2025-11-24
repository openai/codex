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
use crate::common::{dd_fmla, f_fmla};
use crate::double_double::DoubleDouble;
use crate::exponents::auxiliary::fast_ldexp;
use crate::exponents::exp::{EXP_REDUCE_T0, EXP_REDUCE_T1, to_denormal};

#[inline]
fn exp2_poly_dd(z: f64) -> DoubleDouble {
    const C: [(u64, u64); 6] = [
        (0x3bbabc9e3b39873e, 0x3f262e42fefa39ef),
        (0xbae5e43a53e44950, 0x3e4ebfbdff82c58f),
        (0xba0d3a15710d3d83, 0x3d6c6b08d704a0c0),
        (0x3914dd5d2a5e025a, 0x3c83b2ab6fba4e77),
        (0xb83dc47e47beb9dd, 0x3b95d87fe7a66459),
        (0xb744fcd51fcb7640, 0x3aa430912f9fb79d),
    ];

    let mut r = DoubleDouble::quick_mul_f64_add(
        DoubleDouble::from_bit_pair(C[5]),
        z,
        DoubleDouble::from_bit_pair(C[4]),
    );
    r = DoubleDouble::quick_mul_f64_add(r, z, DoubleDouble::from_bit_pair(C[3]));
    r = DoubleDouble::quick_mul_f64_add(r, z, DoubleDouble::from_bit_pair(C[2]));
    r = DoubleDouble::quick_mul_f64_add(r, z, DoubleDouble::from_bit_pair(C[1]));
    DoubleDouble::quick_mul_f64_add(r, z, DoubleDouble::from_bit_pair(C[0]))
}

#[cold]
fn exp2_accurate(x: f64) -> f64 {
    let mut ix = x.to_bits();
    let sx = 4096.0 * x;
    let fx = sx.round_ties_even();
    let z = sx - fx;
    let k: i64 = fx as i64;
    let i1 = k & 0x3f;
    let i0 = (k >> 6) & 0x3f;
    let ie = k >> 12;

    let t0 = DoubleDouble::from_bit_pair(EXP_REDUCE_T0[i0 as usize]);
    let t1 = DoubleDouble::from_bit_pair(EXP_REDUCE_T1[i1 as usize]);
    let dt = DoubleDouble::quick_mult(t0, t1);

    let mut f = exp2_poly_dd(z);
    f = DoubleDouble::quick_mult_f64(f, z);
    if ix <= 0xc08ff00000000000u64 {
        // x >= -1022
        // for -0x1.71547652b82fep-54 <= x <= 0x1.71547652b82fdp-53,
        // exp2(x) round to x to nearest
        if f64::from_bits(0xbc971547652b82fe) <= x && x <= f64::from_bits(0x3ca71547652b82fd) {
            return dd_fmla(x, 0.5, 1.0);
        } else if (k & 0xfff) == 0 {
            // 4096*x rounds to 4096*integer
            let zf = DoubleDouble::from_exact_add(dt.hi, f.hi);
            let zfl = DoubleDouble::from_exact_add(zf.lo, f.lo);
            f.hi = zf.hi;
            f.lo = zfl.hi;
            ix = zfl.hi.to_bits();
            if ix & 0x000fffffffffffff == 0 {
                // fl is a power of 2
                if ((ix >> 52) & 0x7ff) != 0 {
                    // |fl| is Inf
                    let v = zfl.lo.to_bits();
                    let d: i64 = ((((ix as i64) >> 63) ^ ((v as i64) >> 63)) as u64)
                        .wrapping_shl(1)
                        .wrapping_add(1) as i64;
                    ix = ix.wrapping_add(d as u64);
                    f.lo = f64::from_bits(ix);
                }
            }
        } else {
            f = DoubleDouble::quick_mult(f, dt);
            f = DoubleDouble::add(dt, f);
        }
        let hf = DoubleDouble::from_exact_add(f.hi, f.lo);

        fast_ldexp(hf.hi, ie as i32)
    } else {
        ix = 1u64.wrapping_sub(ie as u64).wrapping_shl(52);
        f = DoubleDouble::quick_mult(f, dt);
        f = DoubleDouble::add(dt, f);
        let zve = DoubleDouble::from_exact_add(f64::from_bits(ix), f.hi);
        f.hi = zve.hi;
        f.lo += zve.lo;

        to_denormal(f.to_f64())
    }
}

/// Computes exp2
///
/// Max found ULP 0.5
pub fn f_exp2(x: f64) -> f64 {
    let mut ix = x.to_bits();
    let ax = ix.wrapping_shl(1);
    if ax == 0 {
        return 1.0;
    }
    if ax >= 0x8120000000000000u64 {
        // |x| >= 1024
        if ax > 0xffe0000000000000u64 {
            return x + x; // nan
        }
        if ax == 0xffe0000000000000u64 {
            return if (ix >> 63) != 0 { 0.0 } else { x };
        }
        // +/-inf
        if (ix >> 63) != 0 {
            // x <= -1024
            if ix >= 0xc090cc0000000000u64 {
                // x <= -1075
                const Z: f64 = f64::from_bits(0x0010000000000000);
                return Z * Z;
            }
        } else {
            // x >= 1024
            return f64::from_bits(0x7fe0000000000000) * x;
        }
    }

    // for |x| <= 0x1.71547652b82fep-54, 2^x rounds to 1 to nearest
    // this avoids a spurious underflow in z*z below
    if ax <= 0x792e2a8eca5705fcu64 {
        return 1.0 + f64::copysign(f64::from_bits(0x3c90000000000000), x);
    }

    let m = ix.wrapping_shl(12);
    let ex = (ax >> 53).wrapping_sub(0x3ff);
    let frac = ex >> 63 | m << (ex & 63);
    let sx = 4096.0 * x;
    let fx = sx.round_ties_even();
    let z = sx - fx;
    let z2 = z * z;
    let k = fx as i64;
    let i1 = k & 0x3f;
    let i0 = (k >> 6) & 0x3f;
    let ie = k >> 12;
    let t0 = DoubleDouble::from_bit_pair(EXP_REDUCE_T0[i0 as usize]);
    let t1 = DoubleDouble::from_bit_pair(EXP_REDUCE_T1[i1 as usize]);
    let ti0 = DoubleDouble::quick_mult(t0, t1);
    const C: [u64; 4] = [
        0x3f262e42fefa39ef,
        0x3e4ebfbdff82c58f,
        0x3d6c6b08d73b3e01,
        0x3c83b2ab6fdda001,
    ];
    let tz = ti0.hi * z;
    let mut fh = ti0.hi;

    let p0 = f_fmla(z, f64::from_bits(C[1]), f64::from_bits(C[0]));
    let p1 = f_fmla(z, f64::from_bits(C[3]), f64::from_bits(C[2]));
    let p2 = f_fmla(z2, p1, p0);

    let mut fl = f_fmla(tz, p2, ti0.lo);

    const EPS: f64 = f64::from_bits(0x3c0833beace2b6fe);

    if ix <= 0xc08ff00000000000u64 {
        // x >= -1022
        if frac != 0 {
            let ub = fh + (fl + EPS);
            fh += fl - EPS;
            if ub != fh {
                return exp2_accurate(x);
            }
        }
        fh = fast_ldexp(fh, ie as i32);
    } else {
        // subnormal case
        ix = 1u64.wrapping_sub(ie as u64).wrapping_shl(52);
        let rs = DoubleDouble::from_exact_add(f64::from_bits(ix), fh);
        fl += rs.lo;
        fh = rs.hi;
        if frac != 0 {
            let ub = fh + (fl + EPS);
            fh += fl - EPS;
            if ub != fh {
                return exp2_accurate(x);
            }
        }
        // when 2^x is exact, no underflow should be raised
        fh = to_denormal(fh);
    }
    fh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exp2d() {
        assert_eq!(f_exp2(2.0), 4.0);
        assert_eq!(f_exp2(3.0), 8.0);
        assert_eq!(f_exp2(4.0), 16.0);
        assert!((f_exp2(0.35f64) - 0.35f64.exp2()).abs() < 1e-8);
        assert!((f_exp2(-0.6f64) - (-0.6f64).exp2()).abs() < 1e-8);
        assert_eq!(f_exp2(f64::INFINITY), f64::INFINITY);
        assert_eq!(f_exp2(f64::NEG_INFINITY), 0.);
        assert!(f_exp2(f64::NAN).is_nan());
    }
}
