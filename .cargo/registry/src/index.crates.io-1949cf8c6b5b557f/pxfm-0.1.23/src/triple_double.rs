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
use std::ops::Neg;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TripleDouble {
    pub(crate) hi: f64,
    pub(crate) mid: f64,
    pub(crate) lo: f64,
}

// impl TripleDouble {
//     #[inline]
//     pub(crate) fn mul_dd_add_f64(p0: TripleDouble, p1: DoubleDouble, p2: f64) -> TripleDouble {
//         let q0 = TripleDouble::quick_mul_dd(p0, p1);
//         TripleDouble::add_f64(p2, q0)
//     }
// }

impl TripleDouble {
    // #[inline]
    // pub(crate) fn mul_dd_add(p0: TripleDouble, p1: DoubleDouble, p2: TripleDouble) -> TripleDouble {
    //     let q0 = TripleDouble::quick_mul_dd(p0, p1);
    //     TripleDouble::add(q0, p2)
    // }

    // #[inline]
    // pub(crate) fn mul_dd_add_dd(
    //     p0: TripleDouble,
    //     p1: DoubleDouble,
    //     p2: DoubleDouble,
    // ) -> TripleDouble {
    //     let q0 = TripleDouble::quick_mul_dd(p0, p1);
    //     TripleDouble::add_dd(p2, q0)
    // }
}

impl TripleDouble {
    #[inline]
    pub(crate) fn f64_mul_dd_add(p0: f64, p1: DoubleDouble, p2: TripleDouble) -> TripleDouble {
        let q0 = TripleDouble::from_quick_mult_dd_f64(p1, p0);
        TripleDouble::add(q0, p2)
    }

    #[inline]
    pub(crate) fn f64_mul_add(p0: f64, p1: TripleDouble, p2: TripleDouble) -> TripleDouble {
        let q0 = TripleDouble::quick_mult_f64(p1, p0);
        TripleDouble::add(q0, p2)
    }
}

impl TripleDouble {
    #[inline]
    pub(crate) const fn from_bit_pair(p0: (u64, u64, u64)) -> TripleDouble {
        TripleDouble {
            hi: f64::from_bits(p0.2),
            mid: f64::from_bits(p0.1),
            lo: f64::from_bits(p0.0),
        }
    }
}

#[inline]
fn add12(a: f64, b: f64) -> DoubleDouble {
    DoubleDouble::from_full_exact_add(a, b)
}

#[inline]
fn mul12(a: f64, b: f64) -> DoubleDouble {
    DoubleDouble::from_exact_mult(a, b)
}

#[inline]
pub(crate) fn add22(a: DoubleDouble, b: DoubleDouble) -> DoubleDouble {
    let DoubleDouble { hi: v1, lo: v2 } = DoubleDouble::from_full_exact_add(a.hi, b.hi);
    let v3 = a.lo + b.lo;
    let v4 = v2 + v3;
    DoubleDouble::from_full_exact_add(v1, v4)
}

impl TripleDouble {
    // #[inline]
    // pub(crate) fn from_quick_mult_dd(a: DoubleDouble, b: DoubleDouble) -> TripleDouble {
    //     /*
    //     (rh , t1) ← Mul12 (ah , bh )
    //     (t2, t3) ← Mul12 (ah , bl )
    //     (t4, t5) ← Mul12 (al , bh )
    //     t6 ← al ⊗ bl
    //     (t7, t8) ← Add22 (t2, t3, t4, t5)
    //     (t9, t10) ← Add12 (t1, t6)
    //     (rm , rl ) ← Add22 (t7, t8, t9, t10)
    //      */
    //     let DoubleDouble { hi: rh, lo: t1 } = mul12(a.hi, b.hi);
    //     let r0 = mul12(a.hi, b.lo);
    //     let r1 = mul12(a.lo, b.hi);
    //     let t6 = a.lo * b.lo;
    //     let q0 = add22(r0, r1);
    //     let q1 = add12(t1, t6);
    //     let DoubleDouble { hi: rm, lo: rl } = add22(q0, q1);
    //     TripleDouble {
    //         hi: rh,
    //         mid: rm,
    //         lo: rl,
    //     }
    // }

    #[inline]
    pub(crate) fn from_quick_mult_dd_f64(a: DoubleDouble, b: f64) -> TripleDouble {
        let DoubleDouble { hi: rh, lo: t1 } = mul12(a.hi, b);
        let DoubleDouble { hi: t2, lo: t3 } = mul12(a.lo, b);
        let DoubleDouble { hi: t5, lo: t4 } = add12(t1, t2);
        let t6 = t3 + t4;
        let DoubleDouble { hi: rm, lo: rl } = add12(t5, t6);
        TripleDouble::new(rl, rm, rh)
    }

    #[inline]
    pub(crate) fn quick_mult_f64(a: TripleDouble, b: f64) -> TripleDouble {
        let DoubleDouble { hi: rh, lo: t2 } = mul12(a.hi, b);
        let DoubleDouble { hi: t3, lo: t4 } = mul12(a.mid, b);
        let DoubleDouble { hi: t9, lo: t7 } = add12(t2, t3);
        let t8 = f_fmla(a.lo, b, t4);
        let t10 = t7 + t8;
        let DoubleDouble { hi: rm, lo: rl } = add12(t9, t10);
        TripleDouble::new(rl, rm, rh)
    }

    #[inline]
    pub(crate) fn quick_mult(b: TripleDouble, a: TripleDouble) -> TripleDouble {
        /* Mul12((resh),&_t1,(ah),(bh));
        Mul12(&_t2,&_t3,(ah),(bm));
        Mul12(&_t4,&_t5,(am),(bh));
        Mul12(&_t6,&_t7,(am),(bm));
        _t8 = (ah) * (bl);
        _t9 = (al) * (bh);
        _t10 = (am) * (bl);
        _t11 = (al) * (bm);
        _t12 = _t8 + _t9;
        _t13 = _t10 + _t11;
        Add12Cond(_t14,_t15,_t1,_t6);
        _t16 = _t7 + _t15;
        _t17 = _t12 + _t13;
        _t18 = _t16 + _t17;
        Add12Cond(_t19,_t20,_t14,_t18);
        Add22Cond(&_t21,&_t22,_t2,_t3,_t4,_t5);
        Add22Cond((resm),(resl),_t21,_t22,_t19,_t20);   */
        let DoubleDouble { hi: rh, lo: t1 } = DoubleDouble::from_exact_mult(a.hi, b.hi);
        let DoubleDouble { hi: t2, lo: t3 } = DoubleDouble::from_exact_mult(a.hi, b.mid);
        let DoubleDouble { hi: t4, lo: t5 } = DoubleDouble::from_exact_mult(a.mid, b.hi);
        let DoubleDouble { hi: t6, lo: t7 } = DoubleDouble::from_exact_mult(a.mid, b.mid);
        let t12 = f_fmla(a.hi, b.lo, a.lo * b.hi);
        let t13 = f_fmla(a.mid, b.lo, a.lo * b.mid);
        let DoubleDouble { hi: t14, lo: t15 } = add12(t1, t6);
        let t16 = t7 + t15;
        let t17 = t12 + t13;
        let t18 = t16 + t17;
        let DoubleDouble { hi: t19, lo: t20 } = add12(t14, t18);
        let DoubleDouble { hi: t21, lo: t22 } =
            add22(DoubleDouble::new(t3, t2), DoubleDouble::new(t4, t5));
        let DoubleDouble { hi: rm, lo: rl } =
            add22(DoubleDouble::new(t22, t21), DoubleDouble::new(t20, t19));
        TripleDouble::new(rl, rm, rh)
    }

    #[inline]
    pub(crate) fn quick_mult_dd(b: TripleDouble, a: DoubleDouble) -> TripleDouble {
        /*
        (rh , t1) ← Mul12 (ah , bh )
        (t2, t3) ← Mul12 (ah , bm )
        (t4, t5) ← Mul12 (ah , bl )
        (t6, t7) ← Mul12 (al , bh )
        (t8, t9) ← Mul12 (al , bm )
        t10 ← al ⊗ bl
        (t11, t12) ← Add22 (t2, t3, t4, t5)
        (t13, t14) ← Add22 (t6, t7, t8, t9)
        (t15, t16) ← Add22 (t11, t12, t13, t14)
        (t17, t18) ← Add12 (t1, t10)
        (rm , rl ) ← Add22 (t17, t18, t15, t16)
        */
        let DoubleDouble { hi: rh, lo: t1 } = mul12(a.hi, b.hi);
        let DoubleDouble { hi: t2, lo: t3 } = mul12(a.hi, b.mid);
        let DoubleDouble { hi: t4, lo: t5 } = mul12(a.hi, b.lo);
        let DoubleDouble { hi: t6, lo: t7 } = mul12(a.lo, b.hi);
        let DoubleDouble { hi: t8, lo: t9 } = mul12(a.lo, b.mid);
        let t10 = a.lo * b.lo;
        let z0 = add22(DoubleDouble::new(t3, t2), DoubleDouble::new(t4, t5));
        let z1 = add22(DoubleDouble::new(t6, t7), DoubleDouble::new(t8, t9));
        let q0 = add22(z0, z1);
        let q1 = add12(t1, t10);
        let DoubleDouble { hi: rm, lo: rl } = add22(q1, q0);
        TripleDouble {
            hi: rh,
            mid: rm,
            lo: rl,
        }
    }

    pub(crate) fn add(a: TripleDouble, b: TripleDouble) -> TripleDouble {
        /*
        (rh , t1) ← Add12 (ah , bh )
        (t2, t3) ← Add12 (am , bm )
        (t7, t4) ← Add12 (t1, t2)
        t6 ← al ⊕ bl
        t5 ← t3 ⊕ t4
        t8 ← t5 ⊕ t6
        (rm , rl ) ← Add12 (t7, t8)
                 */
        let DoubleDouble { hi: rh, lo: t1 } = add12(a.hi, b.hi);
        let DoubleDouble { hi: t2, lo: t3 } = add12(a.mid, b.mid);
        let t6 = a.lo + b.lo;
        let DoubleDouble { hi: t7, lo: t4 } = add12(t1, t2);
        let t5 = t3 + t4;
        let t8 = t5 + t6;
        let DoubleDouble { hi: rm, lo: rl } = add12(t7, t8);
        TripleDouble {
            hi: rh,
            mid: rm,
            lo: rl,
        }
    }

    // #[inline]
    // pub(crate) fn add_dd(a: DoubleDouble, b: TripleDouble) -> TripleDouble {
    //     /*
    //     (rh , t1) ← Add12 (ah , bh )
    //     (t2, t3) ← Add12 (al , bm )
    //     (t4, t5) ← Add12 (t1, t2)
    //     t6 ← t3 ⊕ bl
    //     t7 ← t6 ⊕ t5
    //     (rm , rl ) ← Add12 (t4, t7)
    //      */
    //     let DoubleDouble { hi: rh, lo: t1 } = add12(a.hi, b.hi);
    //     let DoubleDouble { hi: t2, lo: t3 } = add12(a.hi, b.mid);
    //     let DoubleDouble { hi: t4, lo: t5 } = add12(t1, t2);
    //     let t6 = t3 + b.lo;
    //     let t7 = t6 + t5;
    //     let DoubleDouble { hi: rm, lo: rl } = add12(t4, t7);
    //     TripleDouble {
    //         hi: rh,
    //         mid: rm,
    //         lo: rl,
    //     }
    // }

    #[inline]
    pub(crate) fn add_f64(a: f64, b: TripleDouble) -> TripleDouble {
        let DoubleDouble { hi: rh, lo: t1 } = add12(a, b.hi);
        let DoubleDouble { hi: t2, lo: t3 } = add12(t1, b.mid);
        let t4 = t3 + b.lo;
        let DoubleDouble { hi: rm, lo: rl } = add12(t2, t4);
        TripleDouble::new(rl, rm, rh)
    }

    // #[inline]
    // pub(crate) fn to_dd(self) -> DoubleDouble {
    //     let DoubleDouble { hi: t1, lo: t2 } = add12(self.hi, self.mid);
    //     let t3 = t2 + self.lo;
    //     DoubleDouble::new(t3, t1)
    // }

    #[inline]
    pub(crate) const fn new(lo: f64, mid: f64, hi: f64) -> Self {
        Self { hi, mid, lo }
    }

    #[inline]
    pub(crate) fn to_f64(self) -> f64 {
        let DoubleDouble { hi: t1, lo: t2 } = add12(self.hi, self.mid);
        let t3 = t2 + self.lo;
        t1 + t3
    }

    #[inline]
    pub(crate) fn from_full_exact_add(a: f64, b: f64) -> Self {
        let DoubleDouble { hi: rh, lo: t1 } = add12(a, b);
        TripleDouble::new(0., t1, rh).renormalize()
    }

    #[inline]
    pub(crate) fn renormalize(self) -> Self {
        /*
           double _t1h, _t1l, _t2l;

        Add12(_t1h, _t1l, (am), (al));
        Add12((*(resh)), _t2l, (ah), (_t1h));
        Add12((*(resm)), (*(resl)), _t2l, _t1l);
         */
        let DoubleDouble { hi: t1h, lo: t1l } = DoubleDouble::from_exact_add(self.mid, self.lo);
        let DoubleDouble { hi: rh, lo: t2l } = DoubleDouble::from_exact_add(self.hi, t1h);
        let zml = DoubleDouble::from_exact_add(t2l, t1l);
        TripleDouble::new(zml.lo, zml.hi, rh)
    }

    #[inline]
    pub(crate) fn recip(self) -> Self {
        /*
              _rec_r1 = 1.0 / (dh);
        Mul12(&_rec_t1,&_rec_t2,_rec_r1,(dh));
        _rec_t3 = _rec_t1 - 1.0;
        Add12Cond(_rec_t4,_rec_t5,_rec_t3,_rec_t2);
        Mul12(&_rec_t6,&_rec_t7,_rec_r1,(dm));
        Add12(_rec_t8,_rec_t9,-1.0,_rec_t6);
        _rec_t10 = _rec_t9 + _rec_t7;
        Add12(_rec_t11,_rec_t12,_rec_t8,_rec_t10);
        _rec_r1 = -_rec_r1;
        Add22Cond(&_rec_t13,&_rec_t14,_rec_t4,_rec_t5,_rec_t11,_rec_t12);
        Mul122(&_rec_r2h,&_rec_r2l,_rec_r1,_rec_t13,_rec_t14);
        Mul233(&_rec_t15,&_rec_t16,&_rec_t17,_rec_r2h,_rec_r2l,(dh),(dm),(dl));
        Renormalize3(&_rec_t18,&_rec_t19,&_rec_t20,_rec_t15,_rec_t16,_rec_t17);
        _rec_t18 = -1.0;
        Mul233(&_rec_t21,&_rec_t22,&_rec_t23,_rec_r2h,_rec_r2l,_rec_t18,_rec_t19,_rec_t20);
        _rec_t21 = -_rec_t21; _rec_t22 = -_rec_t22; _rec_t23 = -_rec_t23;
        Renormalize3((resh),(resm),(resl),_rec_t21,_rec_t22,_rec_t23);
             */
        let mut r1 = 1. / self.hi;
        let DoubleDouble {
            hi: rec_t1,
            lo: rec_t2,
        } = DoubleDouble::from_exact_mult(r1, self.hi);
        let rec_t3 = rec_t1 - 1.0;
        let DoubleDouble {
            hi: rec_t4,
            lo: rec_t5,
        } = add12(rec_t3, rec_t2);
        let DoubleDouble {
            hi: rec_t6,
            lo: rec_t7,
        } = mul12(r1, self.mid);
        let DoubleDouble {
            hi: rec_t8,
            lo: rec_t9,
        } = add12(-1.0, rec_t6);
        let rec_t10 = rec_t9 + rec_t7;
        r1 = -r1;
        let DoubleDouble {
            hi: rec_t11,
            lo: rec_t12,
        } = add12(rec_t8, rec_t10);
        let z0 = add22(
            DoubleDouble::new(rec_t5, rec_t4),
            DoubleDouble::new(rec_t12, rec_t11),
        );
        let r2hl = DoubleDouble::quick_mult_f64(z0, r1);
        let rec15_16_17 = TripleDouble::quick_mult_dd(self, r2hl);
        let rec18_19_20 = rec15_16_17.renormalize();
        let rec_t18 = -1.0;
        let rec_21_22_23 = -TripleDouble::quick_mult_dd(
            TripleDouble::new(rec18_19_20.lo, rec18_19_20.mid, rec_t18),
            r2hl,
        );
        rec_21_22_23.renormalize()
    }
}

impl Neg for TripleDouble {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self::Output {
        TripleDouble::new(-self.lo, -self.mid, -self.hi)
    }
}
