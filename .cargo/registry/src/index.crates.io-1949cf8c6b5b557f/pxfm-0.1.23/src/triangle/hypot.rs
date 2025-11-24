/*
 * // Copyright (c) Radzivon Bartoshyk 9/2025. All rights reserved.
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
use crate::common::{dyad_fmla, f_fmla};
use crate::double_double::DoubleDouble;
use std::hint::black_box;

// case hypot(x,y) >= 2^1024
#[cold]
fn hypot_overflow() -> f64 {
    const Z: f64 = f64::from_bits(0x7fefffffffffffff);
    black_box(Z) + black_box(Z)
}

#[cold]
fn hypot_denorm(a: u64, b: u64) -> f64 {
    let mut a = a;
    let mut b = b;
    let af = (a as i64) as f64;
    let bf = (b as i64) as f64;
    let mut underflow;
    // af and bf are x and y multiplied by 2^1074, thus integers
    a = a.wrapping_shl(1);
    b = b.wrapping_shl(1);
    let mut rm = f_fmla(af, af, bf * bf).sqrt() as u64;
    let mut tm: i64 = rm.wrapping_shl(1) as i64;
    let mut denom: i64 = a
        .wrapping_mul(a)
        .wrapping_add(b.wrapping_mul(b))
        .wrapping_sub((tm as u64).wrapping_mul(tm as u64)) as i64;
    // D = a^2+b^2 - tm^2
    while denom > 2 * tm {
        // tm too small
        denom -= 2 * tm + 1; // (tm+1)^2 = tm^2 + 2*tm + 1
        tm = tm.wrapping_add(1);
    }
    while denom < 0 {
        // tm too large
        denom += 2 * tm - 1; // (tm-1)^2 = tm^2 - 2*tm + 1
        tm = tm.wrapping_sub(1);
    }
    // tm = floor(sqrt(a^2+b^2)) and 0 <= D = a^2+b^2 - tm^2 < 2*tm+1
    // if D=0 and tm is even, the result is exact
    // if D=0 and tm is odd, the result is a midpoint
    let rb: i32 = (tm & 1) as i32; // round bit for rm
    let rb2: i32 = (denom >= tm) as i32; // round bit for tm
    let sb: i32 = (denom != 0) as i32; // sticky bit for rm
    rm = (tm >> 1) as u64; // truncate the low bit
    underflow = rm < 0x10000000000000u64;
    if rb != 0 || sb != 0 {
        let op = 1.0 + f64::from_bits(0x3c90000000000000);
        let om = 1.0 - f64::from_bits(0x3c90000000000000);
        if op == om {
            // rounding to nearest
            if sb != 0 {
                rm = rm.wrapping_add(rb as u64);
                // we have no underflow when rm is now 2^52 and rb2 != 0
                // Remark: we cannot have a^2+b^2 = (tm+1/2)^2 exactly
                // since this would mean a^2+b^2 = tm^2+tm+1/4,
                // thus a^2+b^2 would be an odd multiple of 2^-1077
                // (since ulp(tm) = 2^-1075)
                if rm >> 52 != 0 && rb2 != 0 {
                    underflow = false;
                }
            } else {
                // sticky bit is 0, round bit is 1: underflow doos not change
                rm += rm & 1; // even rounding
            }
        } else if op > 1.0 {
            // rounding upwards
            rm = rm.wrapping_add(1);
            // we have no underflow when rm is now 2^52 and tm was odd
            if (rm >> 52) != 0 && (tm & 1) != 0 {
                underflow = false;
            }
        }
        if underflow {
            // trigger underflow exception _after_ rounding for inexact results
            let trig_uf = black_box(f64::from_bits(0x0010000000000000));
            _ = black_box(black_box(trig_uf) * black_box(trig_uf)); // triggers underflow
        }
    }
    // else the result is exact, and we have no underflow
    f64::from_bits(rm)
}

/* Here the square root is refined by Newton iterations: x^2+y^2 is exact
and fits in a 128-bit integer, so the approximation is squared (which
also fits in a 128-bit integer), compared and adjusted if necessary using
the exact value of x^2+y^2. */
#[cold]
fn hypot_hard(x: f64, y: f64) -> f64 {
    let op = 1.0 + f64::from_bits(0x3c90000000000000);
    let om = 1.0 - f64::from_bits(0x3c90000000000000);
    let mut xi = x.to_bits();
    let yi = y.to_bits();
    let mut bm = (xi & 0x000fffffffffffff) | (1u64 << 52);
    let mut lm = (yi & 0x000fffffffffffff) | (1u64 << 52);
    let be: i32 = (xi >> 52) as i32;
    let le: i32 = (yi >> 52) as i32;
    let ri = f_fmla(x, x, y * y).sqrt().to_bits();
    const BS: u32 = 2;
    let mut rm: u64 = ri & 0x000fffffffffffff;
    let mut re: i32 = ((ri >> 52) as i32).wrapping_sub(0x3ff);
    rm |= 1u64 << 52;
    for _ in 0..3 {
        if rm == (1u64 << 52) {
            rm = 0x001fffffffffffff;
            re = re.wrapping_sub(1);
        } else {
            rm = rm.wrapping_sub(1);
        }
    }
    bm = bm.wrapping_shl(BS);
    let mut m2: u64 = bm.wrapping_mul(bm);
    let de: i32 = be.wrapping_sub(le);
    let mut ls: i32 = (BS as i32).wrapping_sub(de);
    if ls >= 0 {
        lm <<= ls;
        m2 = m2.wrapping_add(lm.wrapping_mul(lm));
    } else {
        let lm2: u128 = (lm as u128).wrapping_mul(lm as u128);
        ls = ls.wrapping_mul(2);
        m2 = m2.wrapping_add((lm2 >> -ls) as u64); // since ls < 0, the shift by -ls is legitimate
        m2 |= ((lm2.wrapping_shl((128 + ls) as u32)) != 0) as u64;
    }
    let k: i32 = (BS as i32).wrapping_add(re);
    let mut denom: i64;
    loop {
        rm += 1 + (rm >= (1u64 << 53)) as u64;
        let tm: u64 = rm.wrapping_shl(k as u32);
        let rm2: u64 = tm.wrapping_mul(tm);
        denom = (m2 as i64).wrapping_sub(rm2 as i64);
        if denom <= 0 {
            break;
        }
    }
    if denom != 0 {
        if op == om {
            let ssm = if rm <= (1u64 << 53) { 1 } else { 0 };
            let tm: u64 = (rm << k) - (1 << (k - ssm));
            denom = (m2 as i64).wrapping_sub(tm.wrapping_mul(tm) as i64);
            if denom != 0 {
                rm = rm.wrapping_add((denom >> 63) as u64);
            } else {
                rm = rm.wrapping_sub(rm & 1);
            }
        } else {
            let ssm = if rm > (1u64 << 53) { 1u32 } else { 0 };
            let pdm = if op == 1. { 1u64 } else { 0 };
            rm = rm.wrapping_sub(pdm.wrapping_shl(ssm));
        }
    }
    if rm >= (1u64 << 53) {
        rm >>= 1;
        re = re.wrapping_add(1);
    }

    let e: i64 = be.wrapping_sub(1).wrapping_add(re) as i64;
    xi = (e as u64).wrapping_shl(52).wrapping_add(rm);
    f64::from_bits(xi)
}

/// Computes hypot
///
/// Max ULP 0.5
pub fn f_hypot(x: f64, y: f64) -> f64 {
    let xi = x.to_bits();
    let yi = y.to_bits();
    let emsk = 0x7ffu64 << 52;
    let mut ex = xi & emsk;
    let mut ey = yi & emsk;
    let mut x = x.abs();
    let mut y = y.abs();
    if ex == emsk || ey == emsk {
        /* Either x or y is NaN or Inf */
        let wx = xi.wrapping_shl(1);
        let wy = yi.wrapping_shl(1);
        let wm = emsk.wrapping_shl(1);
        let ninf: i32 = ((wx == wm) ^ (wy == wm)) as i32;
        let nqnn: i32 = (((wx >> 52) == 0xfff) ^ ((wy >> 52) == 0xfff)) as i32;
        /* ninf is 1 when only one of x and y is +/-Inf
        nqnn is 1 when only one of x and y is qNaN
        IEEE 754 says that hypot(+/-Inf,qNaN)=hypot(qNaN,+/-Inf)=+Inf. */
        if ninf != 0 && nqnn != 0 {
            return if wx == wm { x * x } else { y * y };
        }
        return x + y; /* inf, nan */
    }

    let u = f64::max(x, y);
    let v = f64::min(x, y);
    let mut xd = u.to_bits();
    let mut yd = v.to_bits();
    ey = yd;
    if (ey >> 52) == 0 {
        // y is subnormal
        if (yd) == 0 {
            return f64::from_bits(xd);
        }
        ex = xd;
        if (ex >> 52) == 0 {
            // x is subnormal too
            if ex == 0 {
                return 0.;
            }
            return hypot_denorm(ex, ey);
        }
        let nz = ey.leading_zeros();
        ey = ey.wrapping_shl(nz - 11);
        ey &= u64::MAX >> 12;
        ey = ey.wrapping_sub(((nz as u64).wrapping_sub(12u64)) << 52);
        let t = ey;
        yd = t;
    }

    let de = xd.wrapping_sub(yd);
    if de > (27u64 << 52) {
        return dyad_fmla(f64::from_bits(0x3e40000000000000), v, u);
    }
    let off: i64 = ((0x3ffu64 << 52) as i64).wrapping_sub((xd & emsk) as i64);
    xd = xd.wrapping_add(off as u64);
    yd = yd.wrapping_add(off as u64);
    x = f64::from_bits(xd);
    y = f64::from_bits(yd);
    let x2 = DoubleDouble::from_exact_mult(x, x);
    let y2 = DoubleDouble::from_exact_mult(y, y);
    let r2 = x2.hi + y2.hi;
    let ir2 = 0.5 / r2;
    let dr2 = ((x2.hi - r2) + y2.hi) + (x2.lo + y2.lo);
    let mut th = r2.sqrt();
    let rsqrt = DoubleDouble::from_exact_mult(th, ir2);
    let dz = dr2 - rsqrt.lo;
    let mut tl = rsqrt.hi * dz;
    let p = DoubleDouble::from_exact_add(th, tl);
    th = p.hi;
    tl = p.lo;
    let mut thd = th.to_bits();
    let tld = tl.abs().to_bits();
    ex = thd;
    ey = tld;
    ex &= 0x7ffu64 << 52;
    let aidr = ey.wrapping_add(0x3feu64 << 52).wrapping_sub(ex);
    let mid = (aidr.wrapping_sub(0x3c90000000000000).wrapping_add(16)) >> 5;
    if mid == 0 || aidr < 0x39b0000000000000u64 || aidr > 0x3c9fffffffffff80u64 {
        thd = hypot_hard(x, y).to_bits();
    }
    thd = thd.wrapping_sub(off as u64);
    if thd >= (0x7ffu64 << 52) {
        return hypot_overflow();
    }
    f64::from_bits(thd)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_hypot() {
        assert_eq!(
            f_hypot(2.8480945070593211E-306, 2.1219950320804403E-314),
            2.848094507059321e-306
        );
        assert_eq!(
            f_hypot(3.5601181736115222E-307, 1.0609978954826362E-314),
            3.560118173611524e-307
        );
        assert_eq!(f_hypot(2., 4.), 4.47213595499958);
    }
}
