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
use crate::bits::get_exponent_f64;
#[allow(unused_imports)]
use crate::common::*;
use std::ops::{Mul, Neg};
// https://hal.science/hal-01351529v3/document

#[derive(Copy, Clone, Default, Debug)]
pub(crate) struct DoubleDouble {
    pub(crate) lo: f64,
    pub(crate) hi: f64,
}

impl Neg for DoubleDouble {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self {
            hi: -self.hi,
            lo: -self.lo,
        }
    }
}

impl DoubleDouble {
    #[inline]
    pub(crate) const fn from_bit_pair(pair: (u64, u64)) -> Self {
        Self {
            lo: f64::from_bits(pair.0),
            hi: f64::from_bits(pair.1),
        }
    }

    #[inline]
    pub(crate) const fn new(lo: f64, hi: f64) -> Self {
        DoubleDouble { lo, hi }
    }

    // Non FMA helper
    #[allow(dead_code)]
    #[inline]
    pub(crate) const fn split(a: f64) -> DoubleDouble {
        // CN = 2^N.
        const CN: f64 = (1 << 27) as f64;
        const C: f64 = CN + 1.0;
        let t1 = C * a;
        let t2 = a - t1;
        let r_hi = t1 + t2;
        let r_lo = a - r_hi;
        DoubleDouble::new(r_lo, r_hi)
    }

    // Non FMA helper
    #[allow(dead_code)]
    #[inline]
    fn from_exact_mult_impl_non_fma(asz: DoubleDouble, a: f64, b: f64) -> Self {
        let bs = DoubleDouble::split(b);

        let r_hi = a * b;
        let t1 = asz.hi * bs.hi - r_hi;
        let t2 = asz.hi * bs.lo + t1;
        let t3 = asz.lo * bs.hi + t2;
        let r_lo = asz.lo * bs.lo + t3;
        DoubleDouble::new(r_lo, r_hi)
    }

    // valid only for |a| > b
    #[inline]
    pub(crate) const fn from_exact_add(a: f64, b: f64) -> DoubleDouble {
        let r_hi = a + b;
        let t = r_hi - a;
        let r_lo = b - t;
        DoubleDouble::new(r_lo, r_hi)
    }

    // valid only for |a| > b
    #[inline]
    pub(crate) const fn from_exact_sub(a: f64, b: f64) -> DoubleDouble {
        let r_hi = a - b;
        let t = a - r_hi;
        let r_lo = t - b;
        DoubleDouble::new(r_lo, r_hi)
    }

    #[inline]
    pub(crate) const fn from_full_exact_add(a: f64, b: f64) -> DoubleDouble {
        let r_hi = a + b;
        let t1 = r_hi - a;
        let t2 = r_hi - t1;
        let t3 = b - t1;
        let t4 = a - t2;
        let r_lo = t3 + t4;
        DoubleDouble::new(r_lo, r_hi)
    }

    #[allow(unused)]
    #[inline]
    pub(crate) fn dd_f64_mul_add(a: f64, b: f64, c: f64) -> f64 {
        let ddx2 = DoubleDouble::from_exact_mult(a, b);
        let zv = DoubleDouble::full_add_f64(ddx2, c);
        zv.to_f64()
    }

    #[inline]
    pub(crate) const fn from_full_exact_sub(a: f64, b: f64) -> Self {
        let r_hi = a - b;
        let t1 = r_hi - a;
        let t2 = r_hi - t1;
        let t3 = -b - t1;
        let t4 = a - t2;
        let r_lo = t3 + t4;
        DoubleDouble::new(r_lo, r_hi)
    }

    #[inline]
    pub(crate) fn add(a: DoubleDouble, b: DoubleDouble) -> DoubleDouble {
        let s = a.hi + b.hi;
        let d = s - a.hi;
        let l = ((b.hi - d) + (a.hi + (d - s))) + (a.lo + b.lo);
        DoubleDouble::new(l, s)
    }

    #[inline]
    pub(crate) fn quick_dd_add(a: DoubleDouble, b: DoubleDouble) -> DoubleDouble {
        let DoubleDouble { hi: sh, lo: sl } = DoubleDouble::from_full_exact_add(a.hi, b.hi);
        let v = a.lo + b.lo;
        let w = sl + v;
        DoubleDouble::from_exact_add(sh, w)
    }

    #[inline]
    pub(crate) fn quick_dd_sub(a: DoubleDouble, b: DoubleDouble) -> DoubleDouble {
        let DoubleDouble { hi: sh, lo: sl } = DoubleDouble::from_full_exact_sub(a.hi, b.hi);
        let v = a.lo - b.lo;
        let w = sl + v;
        DoubleDouble::from_exact_add(sh, w)
    }

    #[inline]
    pub(crate) fn full_dd_add(a: DoubleDouble, b: DoubleDouble) -> DoubleDouble {
        let DoubleDouble { hi: sh, lo: sl } = DoubleDouble::from_full_exact_add(a.hi, b.hi);
        let DoubleDouble { hi: th, lo: tl } = DoubleDouble::from_full_exact_add(a.lo, b.lo);
        let c = sl + th;
        let v = DoubleDouble::from_exact_add(sh, c);
        let w = tl + v.lo;
        DoubleDouble::from_exact_add(v.hi, w)
    }

    #[inline]
    pub(crate) fn full_dd_sub(a: DoubleDouble, b: DoubleDouble) -> DoubleDouble {
        DoubleDouble::full_dd_add(a, -b)
    }

    #[inline]
    pub(crate) fn sub(a: DoubleDouble, b: DoubleDouble) -> DoubleDouble {
        let s = a.hi - b.hi;
        let d = s - a.hi;
        let l = ((-b.hi - d) + (a.hi + (d - s))) + (a.lo - b.lo);
        DoubleDouble::new(l, s)
    }

    /// DoubleDouble-style square root for a double-double number
    #[inline]
    pub(crate) fn sqrt(self) -> DoubleDouble {
        let a = self.hi + self.lo;

        if a == 0.0 {
            return DoubleDouble { hi: 0.0, lo: 0.0 };
        }
        if a < 0.0 || a.is_nan() {
            return DoubleDouble {
                hi: f64::NAN,
                lo: 0.0,
            };
        }
        if a.is_infinite() {
            return DoubleDouble {
                hi: f64::INFINITY,
                lo: 0.0,
            };
        }

        let x = a.sqrt();

        let x2 = DoubleDouble::from_exact_mult(x, x);

        // Residual = self - x²
        let mut r = self.hi - x2.hi;
        r += self.lo;
        r -= x2.lo;

        let dx = r / (2.0 * x);
        let hi = x + dx;
        let lo = (x - hi) + dx;

        DoubleDouble { hi, lo }
    }

    /// DoubleDouble-style square root for a double-double number
    #[inline]
    pub(crate) fn fast_sqrt(self) -> DoubleDouble {
        let a = self.hi + self.lo;
        let x = a.sqrt();

        let x2 = DoubleDouble::from_exact_mult(x, x);

        // Residual = self - x²
        let mut r = self.hi - x2.hi;
        r += self.lo;
        r -= x2.lo;

        let dx = r / (2.0 * x);
        let hi = x + dx;
        let lo = (x - hi) + dx;

        DoubleDouble { hi, lo }
    }

    /// `a*b+c`
    ///
    /// *Accurate dot product (Ogita, Rump and Oishi 2004)*
    #[inline]
    pub(crate) fn mul_add_f64(a: DoubleDouble, b: DoubleDouble, c: f64) -> DoubleDouble {
        let DoubleDouble { hi: h, lo: r } = DoubleDouble::quick_mult(a, b);
        let DoubleDouble { hi: p, lo: q } = DoubleDouble::from_full_exact_add(c, h);
        DoubleDouble::new(r + q, p)
    }

    /// `a*b+c`
    ///
    /// *Accurate dot product (Ogita, Rump and Oishi 2004)*
    #[inline]
    pub(crate) fn quick_mul_add_f64(a: DoubleDouble, b: DoubleDouble, c: f64) -> DoubleDouble {
        let DoubleDouble { hi: h, lo: r } = DoubleDouble::quick_mult(a, b);
        let DoubleDouble { hi: p, lo: q } = DoubleDouble::from_exact_add(c, h);
        DoubleDouble::new(r + q, p)
    }

    /// `a*b+c`
    ///
    /// *Accurate dot product (Ogita, Rump and Oishi 2004)*
    #[inline]
    pub(crate) fn mul_f64_add_f64(a: DoubleDouble, b: f64, c: f64) -> DoubleDouble {
        let DoubleDouble { hi: h, lo: r } = DoubleDouble::quick_mult_f64(a, b);
        let DoubleDouble { hi: p, lo: q } = DoubleDouble::from_full_exact_add(c, h);
        DoubleDouble::new(r + q, p)
    }

    /// Accurate reciprocal: 1 / self
    #[inline]
    pub(crate) fn recip_raphson(self) -> DoubleDouble {
        let y0 = DoubleDouble::recip(self);
        let z = DoubleDouble::mul_add_f64(-self, y0, 1.0);
        DoubleDouble::mul_add(y0, z, y0)
    }

    /// Accurate reciprocal: 1 / self
    #[inline]
    pub(crate) fn recip(self) -> DoubleDouble {
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            let y = 1. / self.hi;
            let e1 = f_fmla(-self.hi, y, 1.0);
            let e2 = f_fmla(-self.lo, y, e1);
            let e = y * e2;
            DoubleDouble::new(e, y)
        }
        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let y = 1.0 / self.hi;

            let DoubleDouble { hi: p1, lo: err1 } = DoubleDouble::from_exact_mult(self.hi, y);
            let e1 = (1.0 - p1) - err1;
            let DoubleDouble { hi: p2, lo: err2 } = DoubleDouble::from_exact_mult(self.lo, y);
            let e2 = (e1 - p2) - err2;
            let e = y * e2;

            DoubleDouble::new(e, y)
        }
    }

    #[inline]
    pub(crate) fn from_recip(b: f64) -> Self {
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            let x_hi = 1.0 / b;
            let err = f_fmla(-x_hi, b, 1.0);
            let x_lo = err / b;
            Self::new(x_lo, x_hi)
        }
        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let x_hi = 1.0 / b;
            let prod = Self::from_exact_mult(x_hi, b);
            let err = (1.0 - prod.hi) - prod.lo;
            let x_lo = err / b;
            Self::new(x_lo, x_hi)
        }
    }

    #[inline]
    pub(crate) fn from_quick_recip(b: f64) -> Self {
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            let h = 1.0 / b;
            let hl = f_fmla(h, -b, 1.) * h;
            DoubleDouble::new(hl, h)
        }
        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let h = 1.0 / b;
            let pr = DoubleDouble::from_exact_mult(h, b);
            let err = (1.0 - pr.hi) - pr.lo;
            let hl = err * h;
            DoubleDouble::new(hl, h)
        }
    }

    #[inline]
    pub(crate) fn from_exact_div(a: f64, b: f64) -> Self {
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            let q_hi = a / b;
            let r = f_fmla(-q_hi, b, a);
            let q_lo = r / b;
            Self::new(q_lo, q_hi)
        }

        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let q_hi = a / b;

            let p = DoubleDouble::from_exact_mult(q_hi, b);
            let r = DoubleDouble::from_exact_sub(a, p.hi);
            let r = r.hi + (r.lo - p.lo);
            let q_lo = r / b;

            Self::new(q_lo, q_hi)
        }
    }

    // Resistant to overflow without FMA
    #[inline]
    pub(crate) fn from_exact_safe_div(a: f64, b: f64) -> Self {
        let q_hi = a / b;
        let r = f64::mul_add(-q_hi, b, a);
        let q_lo = r / b;
        Self::new(q_lo, q_hi)
    }

    #[inline]
    pub(crate) fn from_sqrt(x: f64) -> Self {
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            let h = x.sqrt();
            /* h = sqrt(x) * (1 + e1) with |e1| < 2^-52
            thus h^2 = x * (1 + e2) with |e2| < 2^-50.999 */
            let e = -f_fmla(h, h, -x); // exact

            /* e = x - h^2 */
            let l = e / (h + h);
            DoubleDouble::new(l, h)
        }
        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let h = x.sqrt();
            let prod_hh = DoubleDouble::from_exact_mult(h, h);
            let e = (x - prod_hh.hi) - prod_hh.lo; // exact

            /* e = x - h^2 */
            let l = e / (h + h);
            DoubleDouble::new(l, h)
        }
    }

    /// Safe to overflow underflow division using mandatory FMA.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn div_safe_dd_f64(a: DoubleDouble, b: f64) -> Self {
        let q1 = a.hi / b;
        let r = f64::mul_add(-q1, b, a.hi);
        let r = r + a.lo;
        let q2 = r / b;

        DoubleDouble::new(q2, q1)
    }

    #[inline]
    pub(crate) fn div_dd_f64(a: DoubleDouble, b: f64) -> Self {
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            let q1 = a.hi / b;
            let r = f_fmla(-q1, b, a.hi);
            let r = r + a.lo;
            let q2 = r / b;

            DoubleDouble::new(q2, q1)
        }
        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let th = a.hi / b;
            let prod = DoubleDouble::from_exact_mult(th, b);
            let beta_h = a.hi - prod.hi;
            let beta_l = beta_h - prod.lo;
            let beta = beta_l + a.lo;
            let tl = beta / b;
            DoubleDouble::new(tl, th)
        }
    }

    /// Dekker division with one refinement step
    #[inline]
    pub(crate) fn div_dd_f64_newton_raphson(a: DoubleDouble, b: f64) -> Self {
        // Initial estimate q = a / b
        let q = DoubleDouble::div_dd_f64(a, b);

        // One Newton-Raphson refinement step:
        // e = a - q * b
        let qb = DoubleDouble::quick_mult_f64(q, b);
        let e = DoubleDouble::sub(a, qb);
        let e_div_b = DoubleDouble::div_dd_f64(e, b);

        DoubleDouble::add(q, e_div_b)
    }

    // /// Dekker division with two Newton-Raphson refinement steps
    // #[inline]
    // pub(crate) fn div_dd_f64_newton_raphson_2(a: Dekker, b: f64) -> Self {
    //     // First estimate: q = a / b (one round of Dekker division)
    //     let q1 = Dekker::div_dd_f64(a, b);
    //
    //     // First refinement: q2 = q1 + (a - q1 * b) / b
    //     let qb1 = Dekker::quick_mult_f64(q1, b);
    //     let e1 = Dekker::sub(a, qb1);
    //     let dq1 = Dekker::div_dd_f64(e1, b);
    //     let q2 = Dekker::add(q1, dq1);
    //
    //     // Second refinement: q3 = q2 + (a - q2 * b) / b
    //     let qb2 = Dekker::quick_mult_f64(q2, b);
    //     let e2 = Dekker::sub(a, qb2);
    //     let dq2 = Dekker::div_dd_f64(e2, b);
    //
    //     Dekker::add(q2, dq2)
    // }

    // #[inline]
    // pub(crate) fn neg(self) -> Self {
    //     Self {
    //         lo: -self.lo, hi: -self.hi,
    //     }
    // }

    #[inline]
    pub(crate) fn from_f64_div_dd(a: f64, b: DoubleDouble) -> Self {
        let q1 = a / b.hi;

        let prod = DoubleDouble::from_exact_mult(q1, b.hi);
        let prod_lo = f_fmla(q1, b.lo, prod.lo);
        let rem = f_fmla(-1.0, prod.hi, a) - prod_lo;

        let q2 = rem / b.hi;

        DoubleDouble::new(q2, q1)
    }

    // #[inline]
    // pub(crate) fn mla_f64(a: Dekker, b: f64, c: f64) -> Self {
    //     let q = Dekker::mult_f64(a, b);
    //     Dekker::add_f64(q, c)
    // }
    //
    // #[inline]
    // pub(crate) fn mla_dd_f64(a: Dekker, b: Dekker, c: f64) -> Self {
    //     let q = Dekker::quick_mult(a, b);
    //     Dekker::add_f64(q, c)
    // }

    #[inline]
    pub(crate) fn div(a: DoubleDouble, b: DoubleDouble) -> DoubleDouble {
        let q = 1.0 / b.hi;
        let r_hi = a.hi * q;
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            let e_hi = f_fmla(b.hi, -r_hi, a.hi);
            let e_lo = f_fmla(b.lo, -r_hi, a.lo);
            let r_lo = q * (e_hi + e_lo);
            DoubleDouble::new(r_lo, r_hi)
        }

        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let b_hi_r_hi = DoubleDouble::from_exact_mult(b.hi, -r_hi);
            let b_lo_r_hi = DoubleDouble::from_exact_mult(b.lo, -r_hi);
            let e_hi = (a.hi + b_hi_r_hi.hi) + b_hi_r_hi.lo;
            let e_lo = (a.lo + b_lo_r_hi.hi) + b_lo_r_hi.lo;
            let r_lo = q * (e_hi + e_lo);
            DoubleDouble::new(r_lo, r_hi)
        }
    }

    #[inline]
    pub(crate) fn from_exact_mult(a: f64, b: f64) -> Self {
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            let r_hi = a * b;
            let r_lo = f_fmla(a, b, -r_hi);
            DoubleDouble::new(r_lo, r_hi)
        }
        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let splat = DoubleDouble::split(a);
            DoubleDouble::from_exact_mult_impl_non_fma(splat, a, b)
        }
    }

    // #[inline]
    // pub(crate) fn add_f64(&self, other: f64) -> DoubleDouble {
    //     let r = DoubleDouble::from_exact_add(self.hi, other);
    //     Dekker::from_exact_add(r.hi, r.lo + self.lo)
    // }

    // #[inline]
    // pub(crate) fn to_triple(self) -> TripleDouble {
    //     TripleDouble::new(0., self.lo, self.hi)
    // }

    /// Computes `a * b + c`
    /// `b` is an `f64`, `a` and `c` are `DoubleDouble`.
    ///
    /// *Accurate dot product (Ogita, Rump and Oishi 2004)*
    #[inline]
    pub(crate) fn mul_f64_add(a: DoubleDouble, b: f64, c: DoubleDouble) -> Self {
        let DoubleDouble { hi: h, lo: r } = DoubleDouble::quick_mult_f64(a, b);
        let DoubleDouble { hi: p, lo: q } = DoubleDouble::full_add_f64(c, h);
        DoubleDouble::new(r + q, p)
    }

    /// Computes `a * b + c`
    /// `b` is an `f64`, `a` and `c` are `DoubleDouble`.
    ///
    /// *Accurate dot product (Ogita, Rump and Oishi 2004)*
    ///
    /// *Correctness*
    /// |c.hi| > |a.hi * b.hi|
    #[inline]
    pub(crate) fn quick_mul_f64_add(a: DoubleDouble, b: f64, c: DoubleDouble) -> Self {
        let DoubleDouble { hi: h, lo: r } = DoubleDouble::quick_mult_f64(a, b);
        let DoubleDouble { hi: p, lo: q } = DoubleDouble::add_f64(c, h);
        DoubleDouble::new(r + q, p)
    }

    /// Computes `a * b + c`
    ///
    /// *Accurate dot product (Ogita, Rump and Oishi 2004)*
    ///
    /// *Correctness*
    /// |c.hi| > |a.hi * b.hi|
    #[inline]
    pub(crate) fn quick_mul_f64_add_f64(a: DoubleDouble, b: f64, c: f64) -> Self {
        let DoubleDouble { hi: h, lo: r } = DoubleDouble::quick_mult_f64(a, b);
        let DoubleDouble { hi: p, lo: q } = DoubleDouble::from_exact_add(c, h);
        DoubleDouble::new(r + q, p)
    }

    // #[inline]
    // pub(crate) fn mul_f64_add_full(a: DoubleDouble, b: f64, c: DoubleDouble) -> Self {
    //     /*
    //         double _t1, _t2, _t3, _t4, _t5, _t6, _t7, _t8;   \
    //                                                  \
    //         Mul12(&_t1,&_t2,(a),(bh));                       \
    //         Add12(_t3,_t4,(ch),_t1);                         \
    //         _t5 = (bl) * (a);                                \
    //         _t6 = (cl) + _t2;                                \
    //         _t7 = _t5 + _t6;                                 \
    //         _t8 = _t7 + _t4;                                 \
    //         Add12((*(resh)),(*(resl)),_t3,_t8);              \
    //     */
    //     let DoubleDouble { hi: t1, lo: t2 } = DoubleDouble::from_exact_mult(a.hi, b);
    //     let DoubleDouble { hi: t3, lo: t4 } = DoubleDouble::from_full_exact_add(c.hi, t1);
    //     let t5 = a.lo * b;
    //     let t6 = c.lo + t2;
    //     let t7 = t5 + t6;
    //     let t8 = t7 + t4;
    //     DoubleDouble::from_full_exact_add(t3, t8)
    // }

    /// Computes `a * b + c`
    /// `b` is an `f64`, `a` and `c` are `DoubleDouble`.
    ///
    /// *Accurate dot product (Ogita, Rump and Oishi 2004)*
    #[inline]
    pub(crate) fn f64_mul_f64_add(a: f64, b: f64, c: DoubleDouble) -> Self {
        let DoubleDouble { hi: h, lo: r } = DoubleDouble::from_exact_mult(a, b);
        let DoubleDouble { hi: p, lo: q } = DoubleDouble::full_add_f64(c, h);
        DoubleDouble::new(r + q, p)
    }

    // /// Computes `a * b + c`
    // /// `b` is an `f64`, `a` and `c` are `DoubleDouble`.
    // ///
    // /// *Accurate dot product (Ogita, Rump and Oishi 2004)*
    // #[inline]
    // pub(crate) fn single_mul_add(a: f64, b: f64, c: f64) -> Self {
    //     let DoubleDouble { hi: h, lo: r } = DoubleDouble::from_exact_mult(a, b);
    //     let DoubleDouble { hi: p, lo: q } = DoubleDouble::from_full_exact_add(c, h);
    //     DoubleDouble::new(r + q, p)
    // }

    // /// Computes `a * b + c` safe to overflow without FMA
    // /// `b` is an `f64`, `a` and `c` are `DoubleDouble`.
    // ///
    // /// *Accurate dot product (Ogita, Rump and Oishi 2004)*
    // #[inline]
    // pub(crate) fn mul_f64_safe_add(a: DoubleDouble, b: f64, c: DoubleDouble) -> Self {
    //     let DoubleDouble { hi: h, lo: r } = DoubleDouble::quick_mult_safe_f64(a, b);
    //     let DoubleDouble { hi: p, lo: q } = DoubleDouble::full_add_f64(c, h);
    //     DoubleDouble::new(r + q, p)
    // }

    /// `a*b+c`
    ///
    /// *Accurate dot product (Ogita, Rump and Oishi 2004)*
    #[inline]
    pub(crate) fn mul_add(a: DoubleDouble, b: DoubleDouble, c: DoubleDouble) -> Self {
        let DoubleDouble { hi: h, lo: r } = DoubleDouble::quick_mult(a, b);
        let DoubleDouble { hi: p, lo: q } = DoubleDouble::full_add_f64(c, h);
        DoubleDouble::new(r + q, p)
    }

    /// `a*b+c`
    ///
    /// *Accurate dot product (Ogita, Rump and Oishi 2004)*
    ///
    /// *Correctness*
    /// |c.hi| > |a.hi * b.hi|
    #[inline]
    pub(crate) fn quick_mul_add(a: DoubleDouble, b: DoubleDouble, c: DoubleDouble) -> Self {
        let DoubleDouble { hi: h, lo: r } = DoubleDouble::quick_mult(a, b);
        let DoubleDouble { hi: p, lo: q } = DoubleDouble::add_f64(c, h);
        DoubleDouble::new(r + q, p)
    }

    #[inline]
    pub(crate) fn quick_mult(a: DoubleDouble, b: DoubleDouble) -> Self {
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            let mut r = DoubleDouble::from_exact_mult(a.hi, b.hi);
            let t1 = f_fmla(a.hi, b.lo, r.lo);
            let t2 = f_fmla(a.lo, b.hi, t1);
            r.lo = t2;
            r
        }
        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let DoubleDouble { hi: ch, lo: cl1 } = DoubleDouble::from_exact_mult(a.hi, b.hi);
            let tl1 = a.hi * b.lo;
            let tl2 = a.lo * b.hi;
            let cl2 = tl1 + tl2;
            let cl3 = cl1 + cl2;
            DoubleDouble::new(cl3, ch)
        }
    }

    #[inline]
    pub(crate) fn mult(a: DoubleDouble, b: DoubleDouble) -> Self {
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            let DoubleDouble { hi: ch, lo: cl1 } = DoubleDouble::from_exact_mult(a.hi, b.hi);
            let tl0 = a.lo * b.lo;
            let tl1 = f_fmla(a.hi, b.lo, tl0);
            let cl2 = f_fmla(a.lo, b.hi, tl1);
            let cl3 = cl1 + cl2;
            DoubleDouble::from_exact_add(ch, cl3)
        }
        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let DoubleDouble { hi: ch, lo: cl1 } = DoubleDouble::from_exact_mult(a.hi, b.hi);
            let tl1 = a.hi * b.lo;
            let tl2 = a.lo * b.hi;
            let cl2 = tl1 + tl2;
            let cl3 = cl1 + cl2;
            DoubleDouble::from_exact_add(ch, cl3)
        }
    }

    #[inline]
    pub(crate) fn mult_f64(a: DoubleDouble, b: f64) -> Self {
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            let DoubleDouble { hi: ch, lo: cl1 } = DoubleDouble::from_exact_mult(a.hi, b);
            let cl3 = f_fmla(a.lo, b, cl1);
            DoubleDouble::from_exact_add(ch, cl3)
        }
        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let DoubleDouble { hi: ch, lo: cl1 } = DoubleDouble::from_exact_mult(a.hi, b);
            let cl2 = a.lo * b;
            let t = DoubleDouble::from_exact_add(ch, cl2);
            let tl2 = t.lo + cl1;
            DoubleDouble::from_exact_add(t.hi, tl2)
        }
    }

    #[inline]
    pub(crate) fn quick_f64_mult(a: f64, b: DoubleDouble) -> DoubleDouble {
        DoubleDouble::quick_mult_f64(b, a)
    }

    #[inline]
    pub(crate) fn quick_mult_f64(a: DoubleDouble, b: f64) -> Self {
        #[cfg(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        ))]
        {
            let h = b * a.hi;
            let l = f_fmla(b, a.lo, f_fmla(b, a.hi, -h));
            Self { lo: l, hi: h }
        }
        #[cfg(not(any(
            all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "fma"
            ),
            all(target_arch = "aarch64", target_feature = "neon")
        )))]
        {
            let DoubleDouble { hi: ch, lo: cl1 } = DoubleDouble::from_exact_mult(a.hi, b);
            let cl2 = a.lo * b;
            let cl3 = cl1 + cl2;
            DoubleDouble::new(cl3, ch)
        }
    }

    // /// Double-double multiplication safe to overflow without FMA
    // #[inline]
    // pub(crate) fn quick_mult_safe_f64(a: DoubleDouble, b: f64) -> Self {
    //     let h = b * a.hi;
    //     let l = f64::mul_add(b, a.lo, f64::mul_add(b, a.hi, -h));
    //     Self { lo: l, hi: h }
    // }

    /// Valid only |a.hi| > |b|
    #[inline]
    pub(crate) fn add_f64(a: DoubleDouble, b: f64) -> Self {
        let t = DoubleDouble::from_exact_add(a.hi, b);
        let l = a.lo + t.lo;
        Self { lo: l, hi: t.hi }
    }

    #[inline]
    pub(crate) fn full_add_f64(a: DoubleDouble, b: f64) -> Self {
        let t = DoubleDouble::from_full_exact_add(a.hi, b);
        let l = a.lo + t.lo;
        Self { lo: l, hi: t.hi }
    }

    /// Valid only |b| > |a.hi|
    #[inline]
    pub(crate) fn f64_add(b: f64, a: DoubleDouble) -> Self {
        let t = DoubleDouble::from_exact_add(b, a.hi);
        let l = a.lo + t.lo;
        Self { lo: l, hi: t.hi }
    }

    #[inline]
    pub(crate) const fn to_f64(self) -> f64 {
        self.lo + self.hi
    }

    // #[inline]
    // pub(crate) fn from_rsqrt(x: f64) -> DoubleDouble {
    //     let r = DoubleDouble::div_dd_f64(DoubleDouble::from_sqrt(x), x);
    //     let rx = DoubleDouble::quick_mult_safe_f64(r, x);
    //     let drx = DoubleDouble::mul_f64_safe_add(r, x, -rx);
    //     let h = DoubleDouble::mul_add(r, drx, DoubleDouble::mul_add_f64(r, rx, -1.0));
    //     let dr = DoubleDouble::quick_mult(DoubleDouble::quick_mult_f64(r, 0.5), h);
    //     DoubleDouble::add(r, dr)
    // }

    #[inline]
    pub(crate) fn from_rsqrt_fast(x: f64) -> DoubleDouble {
        let sqrt_x = DoubleDouble::from_sqrt(x);
        sqrt_x.recip()
    }
}

impl Mul<DoubleDouble> for DoubleDouble {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: DoubleDouble) -> Self::Output {
        DoubleDouble::quick_mult(self, rhs)
    }
}

/// check if number is valid for Exact mult
#[allow(dead_code)]
#[inline]
pub(crate) fn two_product_compatible(x: f64) -> bool {
    let exp = get_exponent_f64(x);
    !(exp >= 970 || exp <= -970)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_f64_mult() {
        let d1 = 1.1231;
        let d2 = DoubleDouble::new(1e-22, 3.2341);
        let p = DoubleDouble::quick_f64_mult(d1, d2);
        assert_eq!(p.hi, 3.6322177100000004);
        assert_eq!(p.lo, -1.971941841373783e-16);
    }

    #[test]
    fn test_mult_64() {
        let d1 = 1.1231;
        let d2 = DoubleDouble::new(1e-22, 3.2341);
        let p = DoubleDouble::mult_f64(d2, d1);
        assert_eq!(p.hi, 3.6322177100000004);
        assert_eq!(p.lo, -1.971941841373783e-16);
    }

    #[test]
    fn recip_test() {
        let d1 = 1.54352432142;
        let recip = DoubleDouble::new(0., d1).recip();
        assert_eq!(recip.hi, d1.recip());
        assert_ne!(recip.lo, 0.);
    }

    #[test]
    fn from_recip_test() {
        let d1 = 1.54352432142;
        let recip = DoubleDouble::from_recip(d1);
        assert_eq!(recip.hi, d1.recip());
        assert_ne!(recip.lo, 0.);
    }

    #[test]
    fn from_quick_recip_test() {
        let d1 = 1.54352432142;
        let recip = DoubleDouble::from_quick_recip(d1);
        assert_eq!(recip.hi, d1.recip());
        assert_ne!(recip.lo, 0.);
    }
}
