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
use crate::shared_eval::poly_dekker_generic;

static ATAN2F_TABLE: [(u64, u64); 32] = [
    (0x3ff0000000000000, 0xba88c1dac5492248),
    (0xbfd5555555555555, 0xbc755553bf3a2abe),
    (0x3fc999999999999a, 0xbc699deed1ec9071),
    (0xbfc2492492492492, 0xbc5fd99c8d18269a),
    (0x3fbc71c71c71c717, 0xbc2651eee4c4d9d0),
    (0xbfb745d1745d1649, 0xbc5632683d6c44a6),
    (0x3fb3b13b13b11c63, 0x3c5bf69c1f8af41d),
    (0xbfb11111110e6338, 0x3c23c3e431e8bb68),
    (0x3fae1e1e1dc45c4a, 0xbc4be2db05c77bbf),
    (0xbfaaf286b8164b4f, 0x3c2a4673491f0942),
    (0x3fa86185e9ad4846, 0x3c4e12e32d79fcee),
    (0xbfa642c6d5161fae, 0x3c43ce76c1ca03f0),
    (0x3fa47ad6f277e5bf, 0xbc3abd8d85bdb714),
    (0xbfa2f64a2ee8896d, 0x3c2ef87d4b615323),
    (0x3fa1a6a2b31741b5, 0x3c1a5d9d973547ee),
    (0xbfa07fbdad65e0a6, 0xbc265ac07f5d35f4),
    (0x3f9ee9932a9a5f8b, 0x3c2f8b9623f6f55a),
    (0xbf9ce8b5b9584dc6, 0x3c2fe5af96e8ea2d),
    (0x3f9ac9cb288087b7, 0xbc3450cdfceaf5ca),
    (0xbf984b025351f3e6, 0x3c2579561b0d73da),
    (0x3f952f5b8ecdd52b, 0x3c3036bd2c6fba47),
    (0xbf9163a8c44909dc, 0x3c318f735ffb9f16),
    (0x3f8a400dce3eea6f, 0xbc2c90569c0c1b5c),
    (0xbf81caa78ae6db3a, 0xbc24c60f8161ea09),
    (0x3f752672453c0731, 0x3c1834efb598c338),
    (0xbf65850c5be137cf, 0xbc0445fc150ca7f5),
    (0x3f523eb98d22e1ca, 0xbbf388fbaf1d7830),
    (0xbf38f4e974a40741, 0x3bd271198a97da34),
    (0x3f1a5cf2e9cf76e5, 0xbbb887eb4a63b665),
    (0xbef420c270719e32, 0x3b8efd595b27888b),
    (0x3ec3ba2d69b51677, 0xbb64fb06829cdfc7),
    (0xbe829b7e6f676385, 0xbb2a783b6de718fb),
];

#[cold]
fn atan2f_tiny(y: f32, x: f32) -> f32 {
    let dy = y as f64;
    let dx = x as f64;
    let z = dy / dx;
    let mut e = f_fmla(-z, x as f64, y as f64);
    /* z * x + e = y thus y/x = z + e/x */
    const C: f64 = f64::from_bits(0xbfd5555555555555); /* -1/3 rounded to nearest */
    let zz = z * z;
    let cz = C * z;
    e = e / x as f64 + cz * zz;
    let mut t = z.to_bits();
    if (t & 0xfffffffu64) == 0 {
        /* boundary case */
        /* If z and e are of same sign (resp. of different signs), we increase
        (resp. decrease) the significand of t by 1 to avoid a double-rounding
        issue when rounding t.f to binary32. */
        if z * e > 0. {
            t = t.wrapping_add(1);
        } else {
            t = t.wrapping_sub(1);
        }
    }
    f64::from_bits(t) as f32
}

#[allow(clippy::too_many_arguments)]
#[cold]
fn atan2f_refine(ay: u32, ax: u32, y: f32, x: f32, zy: f64, zx: f64, gt: usize, i: u32) -> f32 {
    const PI: f64 = f64::from_bits(0x400921fb54442d18);
    const PI2: f64 = f64::from_bits(0x3ff921fb54442d18);
    const PI2L: f64 = f64::from_bits(0x3c91a62633145c07);
    static OFF: [f64; 8] = [0.0, PI2, PI, PI2, -0.0, -PI2, -PI, -PI2];
    static OFFL: [f64; 8] = [0.0, PI2L, 2. * PI2L, PI2L, -0.0, -PI2L, -2. * PI2L, -PI2L];
    static SGN: [f64; 2] = [1., -1.];
    /* check tiny y/x */
    if ay < ax && ((ax - ay) >> 23 >= 25) {
        return atan2f_tiny(y, x);
    }
    let mut zh;
    let mut zl;
    if gt == 0 {
        zh = zy / zx;
        zl = f_fmla(zh, -zx, zy) / zx;
    } else {
        zh = zx / zy;
        zl = f_fmla(zh, -zy, zx) / zy;
    }
    let z2 = DoubleDouble::quick_mult(DoubleDouble::new(zl, zh), DoubleDouble::new(zl, zh));
    let mut p = poly_dekker_generic(z2, ATAN2F_TABLE);
    zh *= SGN[gt];
    zl *= SGN[gt];
    p = DoubleDouble::quick_mult(DoubleDouble::new(zl, zh), p);
    let sh = p.hi + OFF[i as usize];
    let sl = ((OFF[i as usize] - sh) + p.hi) + p.lo + OFFL[i as usize];
    let rf = sh as f32;
    let th = rf as f64;
    let dh = sh - th;
    let mut tm: f64 = dh + sl;
    let mut tth = th.to_bits();
    if th + th * f64::from_bits(0x3c30000000000000) == th - th * f64::from_bits(0x3c30000000000000)
    {
        tth &= 0x7ffu64 << 52;
        tth = tth.wrapping_sub(24 << 52);
        if tm.abs() > f64::from_bits(tth) {
            tm *= 1.25;
        } else {
            tm *= 0.75;
        }
    }
    let r = th + tm;
    r as f32
}

/// Computes atan2
///
/// Max found ULP 0.49999842
#[inline]
pub fn f_atan2f(y: f32, x: f32) -> f32 {
    const M: [f64; 2] = [0., 1.];
    const PI: f64 = f64::from_bits(0x400921fb54442d18);
    const PI2: f64 = f64::from_bits(0x3ff921fb54442d18);
    const PI2L: f64 = f64::from_bits(0x3c91a62633145c07);
    static OFF: [f64; 8] = [0.0, PI2, PI, PI2, -0.0, -PI2, -PI, -PI2];
    static OFFL: [f64; 8] = [0.0, PI2L, 2. * PI2L, PI2L, -0.0, -PI2L, -2. * PI2L, -PI2L];
    static SGN: [f64; 2] = [1., -1.];
    let tx = x.to_bits();
    let ty = y.to_bits();
    let ux = tx;
    let uy = ty;
    let ax = ux & 0x7fffffff;
    let ay = uy & 0x7fffffff;
    if ay >= (0xff << 23) || ax >= (0xff << 23) {
        // x or y is nan or inf
        /* we use x+y below so that the invalid exception is set
        for (x,y) = (qnan,snan) or (snan,qnan) */
        if ay > (0xff << 23) {
            return x + y;
        } // case y nan
        if ax > (0xff << 23) {
            return x + y;
        } // case x nan
        let yinf = ay == (0xff << 23);
        let xinf = ax == (0xff << 23);
        if yinf & xinf {
            return if (ux >> 31) != 0 {
                (f64::from_bits(0x4002d97c7f3321d2) * SGN[(uy >> 31) as usize]) as f32 // +/-3pi/4
            } else {
                (f64::from_bits(0x3fe921fb54442d18) * SGN[(uy >> 31) as usize]) as f32 // +/-pi/4
            };
        }
        if xinf {
            return if (ux >> 31) != 0 {
                (PI * SGN[(uy >> 31) as usize]) as f32
            } else {
                (0.0 * SGN[(uy >> 31) as usize]) as f32
            };
        }
        if yinf {
            return (PI2 * SGN[(uy >> 31) as usize]) as f32;
        }
    }
    if ay == 0 {
        if ax == 0 {
            let i = (uy >> 31)
                .wrapping_mul(4)
                .wrapping_add((ux >> 31).wrapping_mul(2));
            return if (ux >> 31) != 0 {
                (OFF[i as usize] + OFFL[i as usize]) as f32
            } else {
                OFF[i as usize] as f32
            };
        }
        if (ux >> 31) == 0 {
            return (0.0 * SGN[(uy >> 31) as usize]) as f32;
        }
    }
    let gt = (ay > ax) as usize;
    let i = (uy >> 31)
        .wrapping_mul(4)
        .wrapping_add((ux >> 31).wrapping_mul(2))
        .wrapping_add(gt as u32);

    let zx = x as f64;
    let zy = y as f64;
    let mut z = f_fmla(M[gt], zx, M[1usize.wrapping_sub(gt)] * zy)
        / f_fmla(M[gt], zy, M[1usize.wrapping_sub(gt)] * zx);
    // z = x/y if |y| > |x|, and z = y/x otherwise
    let mut r;

    let d = ax as i32 - ay as i32;
    if d < (27 << 23) && d > (-(27 << 23)) {
        let z2 = z * z;
        let z4 = z2 * z2;
        let z8 = z4 * z4;
        /* z2 cannot underflow, since for |y|=0x1p-149 and |x|=0x1.fffffep+127
        we get |z| > 2^-277 thus z2 > 2^-554, but z4 and z8 might underflow,
        which might give spurious underflow exceptions. */

        const CN: [u64; 7] = [
            0x3ff0000000000000,
            0x40040e0698f94c35,
            0x400248c5da347f0d,
            0x3fed873386572976,
            0x3fc46fa40b20f1d0,
            0x3f833f5e041eed0f,
            0x3f1546bbf28667c5,
        ];
        const CD: [u64; 7] = [
            0x3ff0000000000000,
            0x4006b8b143a3f6da,
            0x4008421201d18ed5,
            0x3ff8221d086914eb,
            0x3fd670657e3a07ba,
            0x3fa0f4951fd1e72d,
            0x3f4b3874b8798286,
        ];

        let mut cn0 = f_fmla(z2, f64::from_bits(CN[1]), f64::from_bits(CN[0]));
        let cn2 = f_fmla(z2, f64::from_bits(CN[3]), f64::from_bits(CN[2]));
        let mut cn4 = f_fmla(z2, f64::from_bits(CN[5]), f64::from_bits(CN[4]));
        let cn6 = f64::from_bits(CN[6]);
        cn0 = f_fmla(z4, cn2, cn0);
        cn4 = f_fmla(z4, cn6, cn4);
        cn0 = f_fmla(z8, cn4, cn0);
        let mut cd0 = f_fmla(z2, f64::from_bits(CD[1]), f64::from_bits(CD[0]));
        let cd2 = f_fmla(z2, f64::from_bits(CD[3]), f64::from_bits(CD[2]));
        let mut cd4 = f_fmla(z2, f64::from_bits(CD[5]), f64::from_bits(CD[4]));
        let cd6 = f64::from_bits(CD[6]);
        cd0 = f_fmla(z4, cd2, cd0);
        cd4 = f_fmla(z4, cd6, cd4);
        cd0 = f_fmla(z8, cd4, cd0);
        r = cn0 / cd0;
    } else {
        r = 1.;
    }
    z *= SGN[gt];
    r = f_fmla(z, r, OFF[i as usize]);
    let res = r.to_bits();
    if ((res.wrapping_add(8)) & 0xfffffff) <= 16 {
        return atan2f_refine(ay, ax, y, x, zy, zx, gt, i);
    }

    r as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_atan2_test() {
        assert_eq!(
            f_atan2f(
                0.000000000000000000000000000000000000017907922,
                170141180000000000000000000000000000000.
            ),
            0.
        );
        assert_eq!(f_atan2f(-3590000000., -15437000.), -1.5750962);
        assert_eq!(f_atan2f(-5., 2.), -1.19029);
    }
}
