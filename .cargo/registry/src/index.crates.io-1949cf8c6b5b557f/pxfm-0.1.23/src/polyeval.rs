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
use crate::common::{f_fmla, f_fmlaf};
use crate::double_double::DoubleDouble;
use crate::dyadic_float::DyadicFloat128;
use std::ops::Mul;

pub(crate) trait PolyevalMla {
    fn polyeval_mla(a: Self, b: Self, c: Self) -> Self;
}

impl PolyevalMla for f64 {
    #[inline(always)]
    fn polyeval_mla(a: Self, b: Self, c: Self) -> Self {
        f_fmla(a, b, c)
    }
}

impl PolyevalMla for f32 {
    #[inline(always)]
    fn polyeval_mla(a: Self, b: Self, c: Self) -> Self {
        f_fmlaf(a, b, c)
    }
}

impl PolyevalMla for DoubleDouble {
    #[inline(always)]
    fn polyeval_mla(a: Self, b: Self, c: Self) -> Self {
        DoubleDouble::mul_add(a, b, c)
    }
}

impl PolyevalMla for DyadicFloat128 {
    #[inline(always)]
    fn polyeval_mla(a: Self, b: Self, c: Self) -> Self {
        c.quick_add(&a.quick_mul(&b))
    }
}

// impl PolyevalMla for DyadicFloat256 {
//     #[inline(always)]
//     fn polyeval_mla(a: Self, b: Self, c: Self) -> Self {
//         c.quick_add(&a.quick_mul(&b))
//     }
// }

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval6<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
) -> T {
    let x2 = x * x;

    let u0 = T::polyeval_mla(x, a5, a4);
    let u1 = T::polyeval_mla(x, a3, a2);
    let u2 = T::polyeval_mla(x, a1, a0);

    let v0 = T::polyeval_mla(x2, u0, u1);

    T::polyeval_mla(x2, v0, u2)
}

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval5<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
// ) -> T {
//     let t2 = T::polyeval_mla(x, a4, a3);
//     let t3 = T::polyeval_mla(x, t2, a2);
//     let t4 = T::polyeval_mla(x, t3, a1);
//     T::polyeval_mla(x, t4, a0)
// }

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval9<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
) -> T {
    let mut acc = a8;
    acc = T::polyeval_mla(x, acc, a7);
    acc = T::polyeval_mla(x, acc, a6);
    acc = T::polyeval_mla(x, acc, a5);
    acc = T::polyeval_mla(x, acc, a4);
    acc = T::polyeval_mla(x, acc, a3);
    acc = T::polyeval_mla(x, acc, a2);
    acc = T::polyeval_mla(x, acc, a1);
    T::polyeval_mla(x, acc, a0)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_estrin_polyeval9<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
) -> T {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;
    let p0 = T::polyeval_mla(x, a1, a0);
    let p1 = T::polyeval_mla(x, a3, a2);
    let p2 = T::polyeval_mla(x, a5, a4);
    let p3 = T::polyeval_mla(x, a7, a6);

    let q0 = T::polyeval_mla(x2, p1, p0);
    let q1 = T::polyeval_mla(x2, p3, p2);
    let r0 = T::polyeval_mla(x4, q1, q0);
    T::polyeval_mla(x8, a8, r0)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval10<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
    a9: T,
) -> T {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;

    let p0 = T::polyeval_mla(x, a1, a0);
    let p1 = T::polyeval_mla(x, a3, a2);
    let p2 = T::polyeval_mla(x, a5, a4);
    let p3 = T::polyeval_mla(x, a7, a6);
    let p4 = T::polyeval_mla(x, a9, a8);

    let q0 = T::polyeval_mla(x2, p1, p0);
    let q1 = T::polyeval_mla(x2, p3, p2);

    let r0 = T::polyeval_mla(x4, q1, q0);
    T::polyeval_mla(x8, p4, r0)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval11<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
    a9: T,
    a10: T,
) -> T {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;

    let q0 = T::polyeval_mla(x, a1, a0);
    let q1 = T::polyeval_mla(x, a3, a2);
    let q2 = T::polyeval_mla(x, a5, a4);
    let q3 = T::polyeval_mla(x, a7, a6);
    let q4 = T::polyeval_mla(x, a9, a8);

    let r0 = T::polyeval_mla(x2, q1, q0);
    let r1 = T::polyeval_mla(x2, q3, q2);

    let s0 = T::polyeval_mla(x4, r1, r0);
    let s1 = T::polyeval_mla(x2, a10, q4);
    T::polyeval_mla(x8, s1, s0)
}

#[inline(always)]
pub(crate) fn f_polyeval3<T: PolyevalMla + Copy>(x: T, a0: T, a1: T, a2: T) -> T {
    T::polyeval_mla(x, T::polyeval_mla(x, a2, a1), a0)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval4<T: PolyevalMla + Copy>(x: T, a0: T, a1: T, a2: T, a3: T) -> T {
    let t2 = T::polyeval_mla(x, a3, a2);
    let t5 = T::polyeval_mla(x, t2, a1);
    T::polyeval_mla(x, t5, a0)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval13<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
    a9: T,
    a10: T,
    a11: T,
    a12: T,
) -> T {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;

    let t0 = T::polyeval_mla(x, a3, a2);
    let t1 = T::polyeval_mla(x, a1, a0);
    let t2 = T::polyeval_mla(x, a7, a6);
    let t3 = T::polyeval_mla(x, a5, a4);
    let t4 = T::polyeval_mla(x, a11, a10);
    let t5 = T::polyeval_mla(x, a9, a8);

    let q0 = T::polyeval_mla(x2, t0, t1);
    let q1 = T::polyeval_mla(x2, t2, t3);

    let q2 = T::polyeval_mla(x2, t4, t5);

    let q3 = a12;

    let r0 = T::polyeval_mla(x4, q1, q0);
    let r1 = T::polyeval_mla(x4, q3, q2);

    T::polyeval_mla(x8, r1, r0)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval12<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
    a9: T,
    a10: T,
    a11: T,
) -> T {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;

    let e0 = T::polyeval_mla(x, a1, a0);
    let e1 = T::polyeval_mla(x, a3, a2);
    let e2 = T::polyeval_mla(x, a5, a4);
    let e3 = T::polyeval_mla(x, a7, a6);
    let e4 = T::polyeval_mla(x, a9, a8);
    let e5 = T::polyeval_mla(x, a11, a10);

    let f0 = T::polyeval_mla(x2, e1, e0);
    let f1 = T::polyeval_mla(x2, e3, e2);
    let f2 = T::polyeval_mla(x2, e5, e4);

    let g0 = T::polyeval_mla(x4, f1, f0);

    T::polyeval_mla(x8, f2, g0)
}

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_horner_polyeval13<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
// ) -> T {
//     let mut acc = a12;
//     acc = T::polyeval_mla(x, acc, a11);
//     acc = T::polyeval_mla(x, acc, a10);
//     acc = T::polyeval_mla(x, acc, a9);
//     acc = T::polyeval_mla(x, acc, a8);
//     acc = T::polyeval_mla(x, acc, a7);
//     acc = T::polyeval_mla(x, acc, a6);
//     acc = T::polyeval_mla(x, acc, a5);
//     acc = T::polyeval_mla(x, acc, a4);
//     acc = T::polyeval_mla(x, acc, a3);
//     acc = T::polyeval_mla(x, acc, a2);
//     acc = T::polyeval_mla(x, acc, a1);
//     T::polyeval_mla(x, acc, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_horner_polyeval14<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
// ) -> T {
//     let mut acc = a13;
//     acc = T::polyeval_mla(x, acc, a12);
//     acc = T::polyeval_mla(x, acc, a11);
//     acc = T::polyeval_mla(x, acc, a10);
//     acc = T::polyeval_mla(x, acc, a9);
//     acc = T::polyeval_mla(x, acc, a8);
//     acc = T::polyeval_mla(x, acc, a7);
//     acc = T::polyeval_mla(x, acc, a6);
//     acc = T::polyeval_mla(x, acc, a5);
//     acc = T::polyeval_mla(x, acc, a4);
//     acc = T::polyeval_mla(x, acc, a3);
//     acc = T::polyeval_mla(x, acc, a2);
//     acc = T::polyeval_mla(x, acc, a1);
//     T::polyeval_mla(x, acc, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_horner_polyeval12<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
// ) -> T {
//     let mut acc = a11;
//     acc = T::polyeval_mla(x, acc, a10);
//     acc = T::polyeval_mla(x, acc, a9);
//     acc = T::polyeval_mla(x, acc, a8);
//     acc = T::polyeval_mla(x, acc, a7);
//     acc = T::polyeval_mla(x, acc, a6);
//     acc = T::polyeval_mla(x, acc, a5);
//     acc = T::polyeval_mla(x, acc, a4);
//     acc = T::polyeval_mla(x, acc, a3);
//     acc = T::polyeval_mla(x, acc, a2);
//     acc = T::polyeval_mla(x, acc, a1);
//     T::polyeval_mla(x, acc, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval11<T: PolyevalMla + Copy>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
// ) -> T {
//     let k0 = T::polyeval_mla(x, a10, a9);
//     let k1 = T::polyeval_mla(x, k0, a8);
//     let z0 = T::polyeval_mla(x, k1, a7);
//     let t0a = T::polyeval_mla(x, z0, a6);
//     let t1 = T::polyeval_mla(x, t0a, a5);
//     let t2 = T::polyeval_mla(x, t1, a4);
//     let t3 = T::polyeval_mla(x, t2, a3);
//     let t4 = T::polyeval_mla(x, t3, a2);
//     let t5 = T::polyeval_mla(x, t4, a1);
//     T::polyeval_mla(x, t5, a0)
// }

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval14<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
    a9: T,
    a10: T,
    a11: T,
    a12: T,
    a13: T,
) -> T {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;

    let g0 = T::polyeval_mla(x, a1, a0);
    let g1 = T::polyeval_mla(x, a3, a2);
    let g2 = T::polyeval_mla(x, a5, a4);
    let g3 = T::polyeval_mla(x, a7, a6);
    let g4 = T::polyeval_mla(x, a9, a8);
    let g5 = T::polyeval_mla(x, a11, a10);
    let g6 = T::polyeval_mla(x, a13, a12);

    let h0 = T::polyeval_mla(x2, g1, g0);
    let h1 = T::polyeval_mla(x2, g3, g2);
    let h2 = T::polyeval_mla(x2, g5, g4);

    let q0 = T::polyeval_mla(x4, h1, h0);
    let q1 = T::polyeval_mla(x4, g6, h2);

    T::polyeval_mla(x8, q1, q0)
}

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval12<T: PolyevalMla + Copy>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
// ) -> T {
//     let t0 = T::polyeval_mla(x, a11, a10);
//     let k0 = T::polyeval_mla(x, t0, a9);
//     let k1 = T::polyeval_mla(x, k0, a8);
//     let z0 = T::polyeval_mla(x, k1, a7);
//     let t0a = T::polyeval_mla(x, z0, a6);
//     let t1 = T::polyeval_mla(x, t0a, a5);
//     let t2 = T::polyeval_mla(x, t1, a4);
//     let t3 = T::polyeval_mla(x, t2, a3);
//     let t4 = T::polyeval_mla(x, t3, a2);
//     let t5 = T::polyeval_mla(x, t4, a1);
//     T::polyeval_mla(x, t5, a0)
// }

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval7<T: PolyevalMla + Copy>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
) -> T {
    let t1 = T::polyeval_mla(x, a6, a5);
    let t2 = T::polyeval_mla(x, t1, a4);
    let t3 = T::polyeval_mla(x, t2, a3);
    let t4 = T::polyeval_mla(x, t3, a2);
    let t5 = T::polyeval_mla(x, t4, a1);
    T::polyeval_mla(x, t5, a0)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_estrin_polyeval7<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
) -> T {
    let x2 = x * x;
    let x4 = x2 * x2;

    let b0 = T::polyeval_mla(x, a1, a0);
    let b1 = T::polyeval_mla(x, a3, a2);
    let b2 = T::polyeval_mla(x, a5, a4);

    let c0 = T::polyeval_mla(x2, b1, b0);
    let c1 = T::polyeval_mla(x2, a6, b2);

    T::polyeval_mla(x4, c1, c0)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval5<T: PolyevalMla + Copy>(x: T, a0: T, a1: T, a2: T, a3: T, a4: T) -> T {
    let mut acc = a4;
    acc = T::polyeval_mla(x, acc, a3);
    acc = T::polyeval_mla(x, acc, a2);
    acc = T::polyeval_mla(x, acc, a1);
    T::polyeval_mla(x, acc, a0)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn f_estrin_polyeval5<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
) -> T {
    let x2 = x * x;
    let p01 = T::polyeval_mla(x, a1, a0);
    let p23 = T::polyeval_mla(x, a3, a2);
    let t = T::polyeval_mla(x2, a4, p23);
    T::polyeval_mla(x2, t, p01)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval8<T: PolyevalMla + Copy>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
) -> T {
    let z0 = T::polyeval_mla(x, a7, a6);
    let t1 = T::polyeval_mla(x, z0, a5);
    let t2 = T::polyeval_mla(x, t1, a4);
    let t3 = T::polyeval_mla(x, t2, a3);
    let t4 = T::polyeval_mla(x, t3, a2);
    let t5 = T::polyeval_mla(x, t4, a1);
    T::polyeval_mla(x, t5, a0)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_estrin_polyeval8<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
) -> T {
    let x2 = x * x;
    let x4 = x2 * x2;

    let p0 = T::polyeval_mla(x, a1, a0);
    let p1 = T::polyeval_mla(x, a3, a2);
    let p2 = T::polyeval_mla(x, a5, a4);
    let p3 = T::polyeval_mla(x, a7, a6);

    let q0 = T::polyeval_mla(x2, p1, p0);
    let q1 = T::polyeval_mla(x2, p3, p2);

    T::polyeval_mla(x4, q1, q0)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval16<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
    a9: T,
    a10: T,
    a11: T,
    a12: T,
    a13: T,
    a14: T,
    a15: T,
) -> T {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;

    let q0 = T::polyeval_mla(x, a1, a0);
    let q1 = T::polyeval_mla(x, a3, a2);
    let q2 = T::polyeval_mla(x, a5, a4);
    let q3 = T::polyeval_mla(x, a7, a6);
    let q4 = T::polyeval_mla(x, a9, a8);
    let q5 = T::polyeval_mla(x, a11, a10);
    let q6 = T::polyeval_mla(x, a13, a12);
    let q7 = T::polyeval_mla(x, a15, a14);

    let r0 = T::polyeval_mla(x2, q1, q0);
    let r1 = T::polyeval_mla(x2, q3, q2);
    let r2 = T::polyeval_mla(x2, q5, q4);
    let r3 = T::polyeval_mla(x2, q7, q6);

    let s0 = T::polyeval_mla(x4, r1, r0);
    let s1 = T::polyeval_mla(x4, r3, r2);

    T::polyeval_mla(x8, s1, s0)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval15<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
    a9: T,
    a10: T,
    a11: T,
    a12: T,
    a13: T,
    a14: T,
) -> T {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;

    let e0 = T::polyeval_mla(x, a1, a0);
    let e1 = T::polyeval_mla(x, a3, a2);
    let e2 = T::polyeval_mla(x, a5, a4);
    let e3 = T::polyeval_mla(x, a7, a6);
    let e4 = T::polyeval_mla(x, a9, a8);
    let e5 = T::polyeval_mla(x, a11, a10);
    let e6 = T::polyeval_mla(x, a13, a12);

    // Level 2
    let f0 = T::polyeval_mla(x2, e1, e0);
    let f1 = T::polyeval_mla(x2, e3, e2);
    let f2 = T::polyeval_mla(x2, e5, e4);
    let f3 = T::polyeval_mla(x2, a14, e6);

    // Level 3
    let g0 = T::polyeval_mla(x4, f1, f0);
    let g1 = T::polyeval_mla(x4, f3, f2);

    // Final
    T::polyeval_mla(x8, g1, g0)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval18<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
    a9: T,
    a10: T,
    a11: T,
    a12: T,
    a13: T,
    a14: T,
    a15: T,
    a16: T,
    a17: T,
) -> T {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;
    let x16 = x8 * x8;

    let q0 = T::polyeval_mla(x, a1, a0);
    let q1 = T::polyeval_mla(x, a3, a2);
    let q2 = T::polyeval_mla(x, a5, a4);
    let q3 = T::polyeval_mla(x, a7, a6);
    let q4 = T::polyeval_mla(x, a9, a8);
    let q5 = T::polyeval_mla(x, a11, a10);
    let q6 = T::polyeval_mla(x, a13, a12);
    let q7 = T::polyeval_mla(x, a15, a14);
    let q8 = T::polyeval_mla(x, a17, a16);

    let r0 = T::polyeval_mla(x2, q1, q0);
    let r1 = T::polyeval_mla(x2, q3, q2);
    let r2 = T::polyeval_mla(x2, q5, q4);
    let r3 = T::polyeval_mla(x2, q7, q6);

    let s0 = T::polyeval_mla(x4, r1, r0);
    let s1 = T::polyeval_mla(x4, r3, r2);

    let t0 = T::polyeval_mla(x8, s1, s0);

    T::polyeval_mla(x16, q8, t0)
}

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval17<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
// ) -> T {
//     let x2 = x * x;
//     let x4 = x2 * x2;
//     let x8 = x4 * x4;
//     let x16 = x8 * x8;
//
//     let p0 = T::polyeval_mla(x, a1, a0);
//     let p1 = T::polyeval_mla(x, a3, a2);
//     let p2 = T::polyeval_mla(x, a5, a4);
//     let p3 = T::polyeval_mla(x, a7, a6);
//     let p4 = T::polyeval_mla(x, a9, a8);
//     let p5 = T::polyeval_mla(x, a11, a10);
//     let p6 = T::polyeval_mla(x, a13, a12);
//     let p7 = T::polyeval_mla(x, a15, a14);
//
//     let q0 = T::polyeval_mla(x2, p1, p0);
//     let q1 = T::polyeval_mla(x2, p3, p2);
//     let q2 = T::polyeval_mla(x2, p5, p4);
//     let q3 = T::polyeval_mla(x2, p7, p6);
//
//     let r0 = T::polyeval_mla(x4, q1, q0);
//     let r1 = T::polyeval_mla(x4, q3, q2);
//
//     let s0 = T::polyeval_mla(x8, r1, r0);
//
//     T::polyeval_mla(x16, a16, s0)
// }

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval19<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
    a9: T,
    a10: T,
    a11: T,
    a12: T,
    a13: T,
    a14: T,
    a15: T,
    a16: T,
    a17: T,
    a18: T,
) -> T {
    // let z000 = T::polyeval_mla(x, a18, a17);
    // let z00 = T::polyeval_mla(x, z000, a16);
    // let z01 = T::polyeval_mla(x, z00, a15);
    // let t1 = T::polyeval_mla(x, z01, a14);
    // let t2 = T::polyeval_mla(x, t1, a13);
    // let t3 = T::polyeval_mla(x, t2, a12);
    // let t4 = T::polyeval_mla(x, t3, a11);
    // let t5 = T::polyeval_mla(x, t4, a10);
    // let t6 = T::polyeval_mla(x, t5, a9);
    // let t7 = T::polyeval_mla(x, t6, a8);
    // let t8 = T::polyeval_mla(x, t7, a7);
    // let t9 = T::polyeval_mla(x, t8, a6);
    // let t10 = T::polyeval_mla(x, t9, a5);
    // let t11 = T::polyeval_mla(x, t10, a4);
    // let t12 = T::polyeval_mla(x, t11, a3);
    // let t13 = T::polyeval_mla(x, t12, a2);
    // let t14 = T::polyeval_mla(x, t13, a1);
    // T::polyeval_mla(x, t14, a0)

    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;
    let x16 = x8 * x8;

    // Level 0: pairs
    let e0 = T::polyeval_mla(x, a1, a0); // a0 + a1·x
    let e1 = T::polyeval_mla(x, a3, a2); // a2 + a3·x
    let e2 = T::polyeval_mla(x, a5, a4);
    let e3 = T::polyeval_mla(x, a7, a6);
    let e4 = T::polyeval_mla(x, a9, a8);
    let e5 = T::polyeval_mla(x, a11, a10);
    let e6 = T::polyeval_mla(x, a13, a12);
    let e7 = T::polyeval_mla(x, a15, a14);
    let e8 = T::polyeval_mla(x, a17, a16);

    // Level 1: combine with x²
    let f0 = T::polyeval_mla(x2, e1, e0);
    let f1 = T::polyeval_mla(x2, e3, e2);
    let f2 = T::polyeval_mla(x2, e5, e4);
    let f3 = T::polyeval_mla(x2, e7, e6);

    // Level 2: combine with x⁴
    let g0 = T::polyeval_mla(x4, f1, f0);
    let g1 = T::polyeval_mla(x4, f3, f2);

    // Level 3: combine with x⁸
    let h0 = T::polyeval_mla(x8, g1, g0);

    // Final: combine with x¹⁶
    let final_poly = T::polyeval_mla(x16, e8, h0);

    // Degree 18: Add a18·x¹⁸
    // This assumes `x18 = x16 * x2`, since x² already computed
    let x18 = x16 * x2;
    T::polyeval_mla(x18, a18, final_poly)
}

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval20<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
// ) -> T {
//     // let z000 = T::polyeval_mla(x, a19, a18);
//     // let z000 = T::polyeval_mla(x, z000, a17);
//     // let z00 = T::polyeval_mla(x, z000, a16);
//     // let z01 = T::polyeval_mla(x, z00, a15);
//     // let t1 = T::polyeval_mla(x, z01, a14);
//     // let t2 = T::polyeval_mla(x, t1, a13);
//     // let t3 = T::polyeval_mla(x, t2, a12);
//     // let t4 = T::polyeval_mla(x, t3, a11);
//     // let t5 = T::polyeval_mla(x, t4, a10);
//     // let t6 = T::polyeval_mla(x, t5, a9);
//     // let t7 = T::polyeval_mla(x, t6, a8);
//     // let t8 = T::polyeval_mla(x, t7, a7);
//     // let t9 = T::polyeval_mla(x, t8, a6);
//     // let t10 = T::polyeval_mla(x, t9, a5);
//     // let t11 = T::polyeval_mla(x, t10, a4);
//     // let t12 = T::polyeval_mla(x, t11, a3);
//     // let t13 = T::polyeval_mla(x, t12, a2);
//     // let t14 = T::polyeval_mla(x, t13, a1);
//     // T::polyeval_mla(x, t14, a0)
//
//     let x2 = x * x;
//     let x4 = x2 * x2;
//     let x8 = x4 * x4;
//     let x16 = x8 * x8;
//
//     // Evaluate groups of 2 terms at a time
//     let e0 = T::polyeval_mla(x, a1, a0);
//     let e1 = T::polyeval_mla(x, a3, a2);
//     let e2 = T::polyeval_mla(x, a5, a4);
//     let e3 = T::polyeval_mla(x, a7, a6);
//     let e4 = T::polyeval_mla(x, a9, a8);
//     let e5 = T::polyeval_mla(x, a11, a10);
//     let e6 = T::polyeval_mla(x, a13, a12);
//     let e7 = T::polyeval_mla(x, a15, a14);
//     let e8 = T::polyeval_mla(x, a17, a16);
//     let e9 = T::polyeval_mla(x, a19, a18);
//
//     // Now build up using higher powers
//     let f0 = T::polyeval_mla(x2, e1, e0); // (e1 * x² + e0)
//     let f1 = T::polyeval_mla(x2, e3, e2);
//     let f2 = T::polyeval_mla(x2, e5, e4);
//     let f3 = T::polyeval_mla(x2, e7, e6);
//     let f4 = T::polyeval_mla(x2, e9, e8);
//
//     // Next level
//     let g0 = T::polyeval_mla(x4, f1, f0);
//     let g1 = T::polyeval_mla(x4, f3, f2);
//
//     // Final levels
//     let h0 = T::polyeval_mla(x8, g1, g0);
//     T::polyeval_mla(x16, f4, h0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_horner_polyeval21<T: PolyevalMla + Copy>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
// ) -> T {
//     let z000 = T::polyeval_mla(x, a20, a19);
//     let z000 = T::polyeval_mla(x, z000, a18);
//     let z000 = T::polyeval_mla(x, z000, a17);
//     let z00 = T::polyeval_mla(x, z000, a16);
//     let z01 = T::polyeval_mla(x, z00, a15);
//     let t1 = T::polyeval_mla(x, z01, a14);
//     let t2 = T::polyeval_mla(x, t1, a13);
//     let t3 = T::polyeval_mla(x, t2, a12);
//     let t4 = T::polyeval_mla(x, t3, a11);
//     let t5 = T::polyeval_mla(x, t4, a10);
//     let t6 = T::polyeval_mla(x, t5, a9);
//     let t7 = T::polyeval_mla(x, t6, a8);
//     let t8 = T::polyeval_mla(x, t7, a7);
//     let t9 = T::polyeval_mla(x, t8, a6);
//     let t10 = T::polyeval_mla(x, t9, a5);
//     let t11 = T::polyeval_mla(x, t10, a4);
//     let t12 = T::polyeval_mla(x, t11, a3);
//     let t13 = T::polyeval_mla(x, t12, a2);
//     let t14 = T::polyeval_mla(x, t13, a1);
//     T::polyeval_mla(x, t14, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval21<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
// ) -> T {
//     // let z000 = T::polyeval_mla(x, a20, a19);
//     // let z000 = T::polyeval_mla(x, z000, a18);
//     // let z000 = T::polyeval_mla(x, z000, a17);
//     // let z00 = T::polyeval_mla(x, z000, a16);
//     // let z01 = T::polyeval_mla(x, z00, a15);
//     // let t1 = T::polyeval_mla(x, z01, a14);
//     // let t2 = T::polyeval_mla(x, t1, a13);
//     // let t3 = T::polyeval_mla(x, t2, a12);
//     // let t4 = T::polyeval_mla(x, t3, a11);
//     // let t5 = T::polyeval_mla(x, t4, a10);
//     // let t6 = T::polyeval_mla(x, t5, a9);
//     // let t7 = T::polyeval_mla(x, t6, a8);
//     // let t8 = T::polyeval_mla(x, t7, a7);
//     // let t9 = T::polyeval_mla(x, t8, a6);
//     // let t10 = T::polyeval_mla(x, t9, a5);
//     // let t11 = T::polyeval_mla(x, t10, a4);
//     // let t12 = T::polyeval_mla(x, t11, a3);
//     // let t13 = T::polyeval_mla(x, t12, a2);
//     // let t14 = T::polyeval_mla(x, t13, a1);
//     // T::polyeval_mla(x, t14, a0)
//
//     // Powers
//     let x2 = x * x;
//     let x4 = x2 * x2;
//     let x8 = x4 * x4;
//     let x16 = x8 * x8;
//
//     // Level 0: smallest groups
//     let e0 = T::polyeval_mla(x, a1, a0);      // a0 + a1*x
//     let e1 = T::polyeval_mla(x, a3, a2);      // a2 + a3*x
//     let e2 = T::polyeval_mla(x, a5, a4);
//     let e3 = T::polyeval_mla(x, a7, a6);
//     let e4 = T::polyeval_mla(x, a9, a8);
//     let e5 = T::polyeval_mla(x, a11, a10);
//     let e6 = T::polyeval_mla(x, a13, a12);
//     let e7 = T::polyeval_mla(x, a15, a14);
//     let e8 = T::polyeval_mla(x, a17, a16);
//     let e9 = T::polyeval_mla(x, a19, a18);    // a18 + a19*x
//
//     // a20 is alone for now
//
//     // Level 1: group by x²
//     let f0 = T::polyeval_mla(x2, e1, e0);     // (e1)*x² + e0
//     let f1 = T::polyeval_mla(x2, e3, e2);
//     let f2 = T::polyeval_mla(x2, e5, e4);
//     let f3 = T::polyeval_mla(x2, e7, e6);
//     let f4 = T::polyeval_mla(x2, e9, e8);
//
//     // Level 2: group by x⁴
//     let g0 = T::polyeval_mla(x4, f1, f0);
//     let g1 = T::polyeval_mla(x4, f3, f2);
//
//     // Level 3: group by x⁸
//     let h0 = T::polyeval_mla(x8, g1, g0);
//
//     // Level 4: final
//     let i0 = T::polyeval_mla(x16, f4, a20);   // (e9 x + a20) * x² → then into x¹⁶
//     T::polyeval_mla(x8, i0, h0)
//
// }

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval22<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
    a9: T,
    a10: T,
    a11: T,
    a12: T,
    a13: T,
    a14: T,
    a15: T,
    a16: T,
    a17: T,
    a18: T,
    a19: T,
    a20: T,
    a21: T,
) -> T {
    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;
    let x16 = x8 * x8;

    let p0 = T::polyeval_mla(x, a1, a0); // a1·x + a0
    let p1 = T::polyeval_mla(x, a3, a2); // a3·x + a2
    let p2 = T::polyeval_mla(x, a5, a4);
    let p3 = T::polyeval_mla(x, a7, a6);
    let p4 = T::polyeval_mla(x, a9, a8);
    let p5 = T::polyeval_mla(x, a11, a10);
    let p6 = T::polyeval_mla(x, a13, a12);
    let p7 = T::polyeval_mla(x, a15, a14);
    let p8 = T::polyeval_mla(x, a17, a16);
    let p9 = T::polyeval_mla(x, a19, a18);
    let p10 = T::polyeval_mla(x, a21, a20);

    let q0 = T::polyeval_mla(x2, p1, p0); // (a3·x + a2)·x² + (a1·x + a0)
    let q1 = T::polyeval_mla(x2, p3, p2);
    let q2 = T::polyeval_mla(x2, p5, p4);
    let q3 = T::polyeval_mla(x2, p7, p6);
    let q4 = T::polyeval_mla(x2, p9, p8);
    let r0 = T::polyeval_mla(x4, q1, q0); // q1·x⁴ + q0
    let r1 = T::polyeval_mla(x4, q3, q2);
    let s0 = T::polyeval_mla(x8, r1, r0); // r1·x⁸ + r0
    let r2 = T::polyeval_mla(x4, p10, q4); // p10·x⁴ + q4
    T::polyeval_mla(x16, r2, s0)
}

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval28<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
//     a27: T,
// ) -> T {
//     let x2 = x * x;
//     let x4 = x2 * x2;
//     let x8 = x4 * x4;
//
//     // Degree 0–3
//     let e0 = T::polyeval_mla(x, a1, a0);
//     let e1 = T::polyeval_mla(x, a3, a2);
//     let p0 = T::polyeval_mla(x2, e1, e0);
//
//     // Degree 4–7
//     let e2 = T::polyeval_mla(x, a5, a4);
//     let e3 = T::polyeval_mla(x, a7, a6);
//     let p1 = T::polyeval_mla(x2, e3, e2);
//
//     // Degree 8–11
//     let e4 = T::polyeval_mla(x, a9, a8);
//     let e5 = T::polyeval_mla(x, a11, a10);
//     let p2 = T::polyeval_mla(x2, e5, e4);
//
//     // Degree 12–15
//     let e6 = T::polyeval_mla(x, a13, a12);
//     let e7 = T::polyeval_mla(x, a15, a14);
//     let p3 = T::polyeval_mla(x2, e7, e6);
//
//     // Degree 16–19
//     let e8 = T::polyeval_mla(x, a17, a16);
//     let e9 = T::polyeval_mla(x, a19, a18);
//     let p4 = T::polyeval_mla(x2, e9, e8);
//
//     // Degree 20–23
//     let e10 = T::polyeval_mla(x, a21, a20);
//     let e11 = T::polyeval_mla(x, a23, a22);
//     let p5 = T::polyeval_mla(x2, e11, e10);
//
//     // Degree 24–27
//     let e12 = T::polyeval_mla(x, a25, a24);
//     let e13 = T::polyeval_mla(x, a27, a26);
//     let p6 = T::polyeval_mla(x2, e13, e12);
//
//     // Group into x⁴
//     let q0 = T::polyeval_mla(x4, p1, p0);
//     let q1 = T::polyeval_mla(x4, p3, p2);
//     let q2 = T::polyeval_mla(x4, p5, p4);
//
//     // Final x⁸ group
//     let r0 = T::polyeval_mla(x8, q1, q0);
//     let r1 = T::polyeval_mla(x8, p6, q2);
//
//     // Final result
//     T::polyeval_mla(x8 * x8, r1, r0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval23<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
// ) -> T {
//     let mut acc = a22;
//     acc = T::polyeval_mla(x, acc, a21);
//     acc = T::polyeval_mla(x, acc, a20);
//     acc = T::polyeval_mla(x, acc, a19);
//     acc = T::polyeval_mla(x, acc, a18);
//     acc = T::polyeval_mla(x, acc, a17);
//     acc = T::polyeval_mla(x, acc, a16);
//     acc = T::polyeval_mla(x, acc, a15);
//     acc = T::polyeval_mla(x, acc, a14);
//     acc = T::polyeval_mla(x, acc, a13);
//     acc = T::polyeval_mla(x, acc, a12);
//     acc = T::polyeval_mla(x, acc, a11);
//     acc = T::polyeval_mla(x, acc, a10);
//     acc = T::polyeval_mla(x, acc, a9);
//     acc = T::polyeval_mla(x, acc, a8);
//     acc = T::polyeval_mla(x, acc, a7);
//     acc = T::polyeval_mla(x, acc, a6);
//     acc = T::polyeval_mla(x, acc, a5);
//     acc = T::polyeval_mla(x, acc, a4);
//     acc = T::polyeval_mla(x, acc, a3);
//     acc = T::polyeval_mla(x, acc, a2);
//     acc = T::polyeval_mla(x, acc, a1);
//     T::polyeval_mla(x, acc, a0)
// }

#[inline(always)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn f_polyeval24<T: PolyevalMla + Copy + Mul<T, Output = T>>(
    x: T,
    a0: T,
    a1: T,
    a2: T,
    a3: T,
    a4: T,
    a5: T,
    a6: T,
    a7: T,
    a8: T,
    a9: T,
    a10: T,
    a11: T,
    a12: T,
    a13: T,
    a14: T,
    a15: T,
    a16: T,
    a17: T,
    a18: T,
    a19: T,
    a20: T,
    a21: T,
    a22: T,
    a23: T,
) -> T {
    // let z000 = T::polyeval_mla(x, a23, a22);
    // let z000 = T::polyeval_mla(x, z000, a21);
    // let z000 = T::polyeval_mla(x, z000, a20);
    // let z000 = T::polyeval_mla(x, z000, a19);
    // let z000 = T::polyeval_mla(x, z000, a18);
    // let z000 = T::polyeval_mla(x, z000, a17);
    // let z00 = T::polyeval_mla(x, z000, a16);
    // let z01 = T::polyeval_mla(x, z00, a15);
    // let t1 = T::polyeval_mla(x, z01, a14);
    // let t2 = T::polyeval_mla(x, t1, a13);
    // let t3 = T::polyeval_mla(x, t2, a12);
    // let t4 = T::polyeval_mla(x, t3, a11);
    // let t5 = T::polyeval_mla(x, t4, a10);
    // let t6 = T::polyeval_mla(x, t5, a9);
    // let t7 = T::polyeval_mla(x, t6, a8);
    // let t8 = T::polyeval_mla(x, t7, a7);
    // let t9 = T::polyeval_mla(x, t8, a6);
    // let t10 = T::polyeval_mla(x, t9, a5);
    // let t11 = T::polyeval_mla(x, t10, a4);
    // let t12 = T::polyeval_mla(x, t11, a3);
    // let t13 = T::polyeval_mla(x, t12, a2);
    // let t14 = T::polyeval_mla(x, t13, a1);
    // T::polyeval_mla(x, t14, a0)

    let x2 = x * x;
    let x4 = x2 * x2;
    let x8 = x4 * x4;
    let x16 = x8 * x8;

    // Group degree 0–1
    let e0 = T::polyeval_mla(x, a1, a0);
    // Group degree 2–3
    let e1 = T::polyeval_mla(x, a3, a2);
    // Group degree 4–5
    let e2 = T::polyeval_mla(x, a5, a4);
    // Group degree 6–7
    let e3 = T::polyeval_mla(x, a7, a6);
    // Group degree 8–9
    let e4 = T::polyeval_mla(x, a9, a8);
    // Group degree 10–11
    let e5 = T::polyeval_mla(x, a11, a10);
    // Group degree 12–13
    let e6 = T::polyeval_mla(x, a13, a12);
    // Group degree 14–15
    let e7 = T::polyeval_mla(x, a15, a14);
    // Group degree 16–17
    let e8 = T::polyeval_mla(x, a17, a16);
    // Group degree 18–19
    let e9 = T::polyeval_mla(x, a19, a18);
    // Group degree 20–21
    let e10 = T::polyeval_mla(x, a21, a20);
    // Group degree 22–23
    let e11 = T::polyeval_mla(x, a23, a22);

    // Now group into x2 terms
    let f0 = T::polyeval_mla(x2, e1, e0);
    let f1 = T::polyeval_mla(x2, e3, e2);
    let f2 = T::polyeval_mla(x2, e5, e4);
    let f3 = T::polyeval_mla(x2, e7, e6);
    let f4 = T::polyeval_mla(x2, e9, e8);
    let f5 = T::polyeval_mla(x2, e11, e10);

    // Now group into x4 terms
    let g0 = T::polyeval_mla(x4, f1, f0);
    let g1 = T::polyeval_mla(x4, f3, f2);
    let g2 = T::polyeval_mla(x4, f5, f4);

    // Now group into x8 terms
    let h0 = T::polyeval_mla(x8, g1, g0);
    let h1 = g2;

    // Final step (x16 term)
    T::polyeval_mla(x16, h1, h0)
}

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval25<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
// ) -> T {
//     let z000 = T::polyeval_mla(x, a24, a23);
//     let z000 = T::polyeval_mla(x, z000, a22);
//     let z000 = T::polyeval_mla(x, z000, a21);
//     let z000 = T::polyeval_mla(x, z000, a20);
//     let z000 = T::polyeval_mla(x, z000, a19);
//     let z000 = T::polyeval_mla(x, z000, a18);
//     let z000 = T::polyeval_mla(x, z000, a17);
//     let z00 = T::polyeval_mla(x, z000, a16);
//     let z01 = T::polyeval_mla(x, z00, a15);
//     let t1 = T::polyeval_mla(x, z01, a14);
//     let t2 = T::polyeval_mla(x, t1, a13);
//     let t3 = T::polyeval_mla(x, t2, a12);
//     let t4 = T::polyeval_mla(x, t3, a11);
//     let t5 = T::polyeval_mla(x, t4, a10);
//     let t6 = T::polyeval_mla(x, t5, a9);
//     let t7 = T::polyeval_mla(x, t6, a8);
//     let t8 = T::polyeval_mla(x, t7, a7);
//     let t9 = T::polyeval_mla(x, t8, a6);
//     let t10 = T::polyeval_mla(x, t9, a5);
//     let t11 = T::polyeval_mla(x, t10, a4);
//     let t12 = T::polyeval_mla(x, t11, a3);
//     let t13 = T::polyeval_mla(x, t12, a2);
//     let t14 = T::polyeval_mla(x, t13, a1);
//     T::polyeval_mla(x, t14, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval26<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
// ) -> T {
//     let x2 = x * x;
//     let x4 = x2 * x2;
//     let x8 = x4 * x4;
//     let x16 = x8 * x8;
//
//     let y0 = T::polyeval_mla(x, a1, a0);
//     let y1 = T::polyeval_mla(x, a3, a2);
//     let y2 = T::polyeval_mla(x, a5, a4);
//     let y3 = T::polyeval_mla(x, a7, a6);
//     let y4 = T::polyeval_mla(x, a9, a8);
//     let y5 = T::polyeval_mla(x, a11, a10);
//     let y6 = T::polyeval_mla(x, a13, a12);
//     let y7 = T::polyeval_mla(x, a15, a14);
//     let y8 = T::polyeval_mla(x, a17, a16);
//     let y9 = T::polyeval_mla(x, a19, a18);
//     let y10 = T::polyeval_mla(x, a21, a20);
//     let y11 = T::polyeval_mla(x, a23, a22);
//     let y12 = T::polyeval_mla(x, a25, a24);
//
//     let z0 = T::polyeval_mla(x2, y1, y0);
//     let z1 = T::polyeval_mla(x2, y3, y2);
//     let z2 = T::polyeval_mla(x2, y5, y4);
//     let z3 = T::polyeval_mla(x2, y7, y6);
//     let z4 = T::polyeval_mla(x2, y9, y8);
//     let z5 = T::polyeval_mla(x2, y11, y10);
//
//     let w0 = T::polyeval_mla(x4, z1, z0);
//     let w1 = T::polyeval_mla(x4, z3, z2);
//     let w2 = T::polyeval_mla(x4, z5, z4);
//
//     let v0 = T::polyeval_mla(x8, w1, w0);
//     let v1 = T::polyeval_mla(x8, y12, w2);
//
//     T::polyeval_mla(x16, v1, v0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval27<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
// ) -> T {
//     let z000 = T::polyeval_mla(x, a26, a25);
//     let z000 = T::polyeval_mla(x, z000, a24);
//     let z000 = T::polyeval_mla(x, z000, a23);
//     let z000 = T::polyeval_mla(x, z000, a22);
//     let z000 = T::polyeval_mla(x, z000, a21);
//     let z000 = T::polyeval_mla(x, z000, a20);
//     let z000 = T::polyeval_mla(x, z000, a19);
//     let z000 = T::polyeval_mla(x, z000, a18);
//     let z000 = T::polyeval_mla(x, z000, a17);
//     let z00 = T::polyeval_mla(x, z000, a16);
//     let z01 = T::polyeval_mla(x, z00, a15);
//     let t1 = T::polyeval_mla(x, z01, a14);
//     let t2 = T::polyeval_mla(x, t1, a13);
//     let t3 = T::polyeval_mla(x, t2, a12);
//     let t4 = T::polyeval_mla(x, t3, a11);
//     let t5 = T::polyeval_mla(x, t4, a10);
//     let t6 = T::polyeval_mla(x, t5, a9);
//     let t7 = T::polyeval_mla(x, t6, a8);
//     let t8 = T::polyeval_mla(x, t7, a7);
//     let t9 = T::polyeval_mla(x, t8, a6);
//     let t10 = T::polyeval_mla(x, t9, a5);
//     let t11 = T::polyeval_mla(x, t10, a4);
//     let t12 = T::polyeval_mla(x, t11, a3);
//     let t13 = T::polyeval_mla(x, t12, a2);
//     let t14 = T::polyeval_mla(x, t13, a1);
//     T::polyeval_mla(x, t14, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval30<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
//     a27: T,
//     a28: T,
//     a29: T,
// ) -> T {
//     let x2 = x * x;
//     let x4 = x2 * x2;
//     let x8 = x4 * x4;
//     let x16 = x8 * x8;
//
//     // Degree 0–1
//     let e0 = T::polyeval_mla(x, a1, a0);
//     // Degree 2–3
//     let e1 = T::polyeval_mla(x, a3, a2);
//     // Degree 4–5
//     let e2 = T::polyeval_mla(x, a5, a4);
//     // Degree 6–7
//     let e3 = T::polyeval_mla(x, a7, a6);
//     // Degree 8–9
//     let e4 = T::polyeval_mla(x, a9, a8);
//     // Degree 10–11
//     let e5 = T::polyeval_mla(x, a11, a10);
//     // Degree 12–13
//     let e6 = T::polyeval_mla(x, a13, a12);
//     // Degree 14–15
//     let e7 = T::polyeval_mla(x, a15, a14);
//
//     // Combine with x²
//     let f0 = T::polyeval_mla(x2, e1, e0); // deg 0–3
//     let f1 = T::polyeval_mla(x2, e3, e2); // deg 4–7
//     let f2 = T::polyeval_mla(x2, e5, e4); // deg 8–11
//     let f3 = T::polyeval_mla(x2, e7, e6); // deg 12–15
//
//     // Combine with x⁴
//     let g0 = T::polyeval_mla(x4, f1, f0); // deg 0–7
//     let g1 = T::polyeval_mla(x4, f3, f2); // deg 8–15
//
//     // Degree 16–17
//     let e8 = T::polyeval_mla(x, a17, a16);
//     // Degree 18–19
//     let e9 = T::polyeval_mla(x, a19, a18);
//     // Degree 20–21
//     let e10 = T::polyeval_mla(x, a21, a20);
//     // Degree 22–23
//     let e11 = T::polyeval_mla(x, a23, a22);
//     // Degree 24–25
//     let e12 = T::polyeval_mla(x, a25, a24);
//     // Degree 26–27
//     let e13 = T::polyeval_mla(x, a27, a26);
//     // Degree 28–29
//     let e14 = T::polyeval_mla(x, a29, a28);
//
//     // Combine with x²
//     let f4 = T::polyeval_mla(x2, e9, e8); // deg 16–19
//     let f5 = T::polyeval_mla(x2, e11, e10); // deg 20–23
//     let f6 = T::polyeval_mla(x2, e13, e12); // deg 24–27
//
//     // Combine remaining term (28–29)
//     let f7 = e14;
//
//     // Combine with x⁴
//     let g2 = T::polyeval_mla(x4, f5, f4); // deg 16–23
//     let g3 = T::polyeval_mla(x4, f7, f6); // deg 24–29
//
//     // Combine with x⁸
//     let h0 = T::polyeval_mla(x8, g1, g0); // deg 0–15
//     let h1 = T::polyeval_mla(x8, g3, g2); // deg 16–29
//
//     // Final combination with x¹⁶
//     T::polyeval_mla(x16, h1, h0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_horner_polyeval30<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
//     a27: T,
//     a28: T,
//     a29: T,
// ) -> T {
//     let mut acc = a29;
//     acc = T::polyeval_mla(x, acc, a28);
//     acc = T::polyeval_mla(x, acc, a27);
//     acc = T::polyeval_mla(x, acc, a26);
//     acc = T::polyeval_mla(x, acc, a25);
//     acc = T::polyeval_mla(x, acc, a24);
//     acc = T::polyeval_mla(x, acc, a23);
//     acc = T::polyeval_mla(x, acc, a22);
//     acc = T::polyeval_mla(x, acc, a21);
//     acc = T::polyeval_mla(x, acc, a20);
//     acc = T::polyeval_mla(x, acc, a19);
//     acc = T::polyeval_mla(x, acc, a18);
//     acc = T::polyeval_mla(x, acc, a17);
//     acc = T::polyeval_mla(x, acc, a16);
//     acc = T::polyeval_mla(x, acc, a15);
//     acc = T::polyeval_mla(x, acc, a14);
//     acc = T::polyeval_mla(x, acc, a13);
//     acc = T::polyeval_mla(x, acc, a12);
//     acc = T::polyeval_mla(x, acc, a11);
//     acc = T::polyeval_mla(x, acc, a10);
//     acc = T::polyeval_mla(x, acc, a9);
//     acc = T::polyeval_mla(x, acc, a8);
//     acc = T::polyeval_mla(x, acc, a7);
//     acc = T::polyeval_mla(x, acc, a6);
//     acc = T::polyeval_mla(x, acc, a5);
//     acc = T::polyeval_mla(x, acc, a4);
//     acc = T::polyeval_mla(x, acc, a3);
//     acc = T::polyeval_mla(x, acc, a2);
//     acc = T::polyeval_mla(x, acc, a1);
//     T::polyeval_mla(x, acc, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval31<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
//     a27: T,
//     a28: T,
//     a29: T,
//     a30: T,
// ) -> T {
//     let z000 = T::polyeval_mla(x, a30, a29);
//     let z000 = T::polyeval_mla(x, z000, a28);
//     let z000 = T::polyeval_mla(x, z000, a27);
//     let z000 = T::polyeval_mla(x, z000, a26);
//     let z000 = T::polyeval_mla(x, z000, a25);
//     let z000 = T::polyeval_mla(x, z000, a24);
//     let z000 = T::polyeval_mla(x, z000, a23);
//     let z000 = T::polyeval_mla(x, z000, a22);
//     let z000 = T::polyeval_mla(x, z000, a21);
//     let z000 = T::polyeval_mla(x, z000, a20);
//     let z000 = T::polyeval_mla(x, z000, a19);
//     let z000 = T::polyeval_mla(x, z000, a18);
//     let z000 = T::polyeval_mla(x, z000, a17);
//     let z00 = T::polyeval_mla(x, z000, a16);
//     let z01 = T::polyeval_mla(x, z00, a15);
//     let t1 = T::polyeval_mla(x, z01, a14);
//     let t2 = T::polyeval_mla(x, t1, a13);
//     let t3 = T::polyeval_mla(x, t2, a12);
//     let t4 = T::polyeval_mla(x, t3, a11);
//     let t5 = T::polyeval_mla(x, t4, a10);
//     let t6 = T::polyeval_mla(x, t5, a9);
//     let t7 = T::polyeval_mla(x, t6, a8);
//     let t8 = T::polyeval_mla(x, t7, a7);
//     let t9 = T::polyeval_mla(x, t8, a6);
//     let t10 = T::polyeval_mla(x, t9, a5);
//     let t11 = T::polyeval_mla(x, t10, a4);
//     let t12 = T::polyeval_mla(x, t11, a3);
//     let t13 = T::polyeval_mla(x, t12, a2);
//     let t14 = T::polyeval_mla(x, t13, a1);
//     T::polyeval_mla(x, t14, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval33<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
//     a27: T,
//     a28: T,
//     a29: T,
//     a30: T,
//     a31: T,
//     a32: T,
// ) -> T {
//     let z000 = T::polyeval_mla(x, a32, a31);
//     let z000 = T::polyeval_mla(x, z000, a30);
//     let z000 = T::polyeval_mla(x, z000, a29);
//     let z000 = T::polyeval_mla(x, z000, a28);
//     let z000 = T::polyeval_mla(x, z000, a27);
//     let z000 = T::polyeval_mla(x, z000, a26);
//     let z000 = T::polyeval_mla(x, z000, a25);
//     let z000 = T::polyeval_mla(x, z000, a24);
//     let z000 = T::polyeval_mla(x, z000, a23);
//     let z000 = T::polyeval_mla(x, z000, a22);
//     let z000 = T::polyeval_mla(x, z000, a21);
//     let z000 = T::polyeval_mla(x, z000, a20);
//     let z000 = T::polyeval_mla(x, z000, a19);
//     let z000 = T::polyeval_mla(x, z000, a18);
//     let z000 = T::polyeval_mla(x, z000, a17);
//     let z00 = T::polyeval_mla(x, z000, a16);
//     let z01 = T::polyeval_mla(x, z00, a15);
//     let t1 = T::polyeval_mla(x, z01, a14);
//     let t2 = T::polyeval_mla(x, t1, a13);
//     let t3 = T::polyeval_mla(x, t2, a12);
//     let t4 = T::polyeval_mla(x, t3, a11);
//     let t5 = T::polyeval_mla(x, t4, a10);
//     let t6 = T::polyeval_mla(x, t5, a9);
//     let t7 = T::polyeval_mla(x, t6, a8);
//     let t8 = T::polyeval_mla(x, t7, a7);
//     let t9 = T::polyeval_mla(x, t8, a6);
//     let t10 = T::polyeval_mla(x, t9, a5);
//     let t11 = T::polyeval_mla(x, t10, a4);
//     let t12 = T::polyeval_mla(x, t11, a3);
//     let t13 = T::polyeval_mla(x, t12, a2);
//     let t14 = T::polyeval_mla(x, t13, a1);
//     T::polyeval_mla(x, t14, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval29<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
//     a27: T,
//     a28: T,
// ) -> T {
//     let x2 = x * x;
//     let x4 = x2 * x2;
//     let x8 = x4 * x4;
//
//     // Level 0
//     let e0 = T::polyeval_mla(x, a1, a0);
//     let e1 = T::polyeval_mla(x, a3, a2);
//     let e2 = T::polyeval_mla(x, a5, a4);
//     let e3 = T::polyeval_mla(x, a7, a6);
//     let e4 = T::polyeval_mla(x, a9, a8);
//     let e5 = T::polyeval_mla(x, a11, a10);
//     let e6 = T::polyeval_mla(x, a13, a12);
//     let e7 = T::polyeval_mla(x, a15, a14);
//     let e8 = T::polyeval_mla(x, a17, a16);
//     let e9 = T::polyeval_mla(x, a19, a18);
//     let e10 = T::polyeval_mla(x, a21, a20);
//     let e11 = T::polyeval_mla(x, a23, a22);
//     let e12 = T::polyeval_mla(x, a25, a24);
//     let e13 = T::polyeval_mla(x, a27, a26);
//     let e14 = a28; // single term left
//
//     // Level 1
//     let f0 = T::polyeval_mla(x2, e1, e0); // e1*x² + e0
//     let f1 = T::polyeval_mla(x2, e3, e2);
//     let f2 = T::polyeval_mla(x2, e5, e4);
//     let f3 = T::polyeval_mla(x2, e7, e6);
//     let f4 = T::polyeval_mla(x2, e9, e8);
//     let f5 = T::polyeval_mla(x2, e11, e10);
//     let f6 = T::polyeval_mla(x2, e13, e12);
//     let f7 = e14; // promote
//
//     // Level 2
//     let g0 = T::polyeval_mla(x4, f1, f0);
//     let g1 = T::polyeval_mla(x4, f3, f2);
//     let g2 = T::polyeval_mla(x4, f5, f4);
//     let g3 = T::polyeval_mla(x4, f7, f6);
//
//     // Level 3
//     let h0 = T::polyeval_mla(x8, g1, g0);
//     let h1 = T::polyeval_mla(x8, g3, g2);
//
//     // Final level
//     let x16 = x8 * x8;
//     T::polyeval_mla(x16, h1, h0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval37<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
//     a27: T,
//     a28: T,
//     a29: T,
//     a30: T,
//     a31: T,
//     a32: T,
//     a33: T,
//     a34: T,
//     a35: T,
//     a36: T,
// ) -> T {
//     let z000 = T::polyeval_mla(x, a36, a35);
//     let z000 = T::polyeval_mla(x, z000, a34);
//     let z000 = T::polyeval_mla(x, z000, a33);
//     let z000 = T::polyeval_mla(x, z000, a32);
//     let z000 = T::polyeval_mla(x, z000, a31);
//     let z000 = T::polyeval_mla(x, z000, a30);
//     let z000 = T::polyeval_mla(x, z000, a29);
//     let z000 = T::polyeval_mla(x, z000, a28);
//     let z000 = T::polyeval_mla(x, z000, a27);
//     let z000 = T::polyeval_mla(x, z000, a26);
//     let z000 = T::polyeval_mla(x, z000, a25);
//     let z000 = T::polyeval_mla(x, z000, a24);
//     let z000 = T::polyeval_mla(x, z000, a23);
//     let z000 = T::polyeval_mla(x, z000, a22);
//     let z000 = T::polyeval_mla(x, z000, a21);
//     let z000 = T::polyeval_mla(x, z000, a20);
//     let z000 = T::polyeval_mla(x, z000, a19);
//     let z000 = T::polyeval_mla(x, z000, a18);
//     let z000 = T::polyeval_mla(x, z000, a17);
//     let z00 = T::polyeval_mla(x, z000, a16);
//     let z01 = T::polyeval_mla(x, z00, a15);
//     let t1 = T::polyeval_mla(x, z01, a14);
//     let t2 = T::polyeval_mla(x, t1, a13);
//     let t3 = T::polyeval_mla(x, t2, a12);
//     let t4 = T::polyeval_mla(x, t3, a11);
//     let t5 = T::polyeval_mla(x, t4, a10);
//     let t6 = T::polyeval_mla(x, t5, a9);
//     let t7 = T::polyeval_mla(x, t6, a8);
//     let t8 = T::polyeval_mla(x, t7, a7);
//     let t9 = T::polyeval_mla(x, t8, a6);
//     let t10 = T::polyeval_mla(x, t9, a5);
//     let t11 = T::polyeval_mla(x, t10, a4);
//     let t12 = T::polyeval_mla(x, t11, a3);
//     let t13 = T::polyeval_mla(x, t12, a2);
//     let t14 = T::polyeval_mla(x, t13, a1);
//     T::polyeval_mla(x, t14, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval36<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
//     a27: T,
//     a28: T,
//     a29: T,
//     a30: T,
//     a31: T,
//     a32: T,
//     a33: T,
//     a34: T,
//     a35: T,
// ) -> T {
//     let z000 = T::polyeval_mla(x, a35, a34);
//     let z000 = T::polyeval_mla(x, z000, a33);
//     let z000 = T::polyeval_mla(x, z000, a32);
//     let z000 = T::polyeval_mla(x, z000, a31);
//     let z000 = T::polyeval_mla(x, z000, a30);
//     let z000 = T::polyeval_mla(x, z000, a29);
//     let z000 = T::polyeval_mla(x, z000, a28);
//     let z000 = T::polyeval_mla(x, z000, a27);
//     let z000 = T::polyeval_mla(x, z000, a26);
//     let z000 = T::polyeval_mla(x, z000, a25);
//     let z000 = T::polyeval_mla(x, z000, a24);
//     let z000 = T::polyeval_mla(x, z000, a23);
//     let z000 = T::polyeval_mla(x, z000, a22);
//     let z000 = T::polyeval_mla(x, z000, a21);
//     let z000 = T::polyeval_mla(x, z000, a20);
//     let z000 = T::polyeval_mla(x, z000, a19);
//     let z000 = T::polyeval_mla(x, z000, a18);
//     let z000 = T::polyeval_mla(x, z000, a17);
//     let z00 = T::polyeval_mla(x, z000, a16);
//     let z01 = T::polyeval_mla(x, z00, a15);
//     let t1 = T::polyeval_mla(x, z01, a14);
//     let t2 = T::polyeval_mla(x, t1, a13);
//     let t3 = T::polyeval_mla(x, t2, a12);
//     let t4 = T::polyeval_mla(x, t3, a11);
//     let t5 = T::polyeval_mla(x, t4, a10);
//     let t6 = T::polyeval_mla(x, t5, a9);
//     let t7 = T::polyeval_mla(x, t6, a8);
//     let t8 = T::polyeval_mla(x, t7, a7);
//     let t9 = T::polyeval_mla(x, t8, a6);
//     let t10 = T::polyeval_mla(x, t9, a5);
//     let t11 = T::polyeval_mla(x, t10, a4);
//     let t12 = T::polyeval_mla(x, t11, a3);
//     let t13 = T::polyeval_mla(x, t12, a2);
//     let t14 = T::polyeval_mla(x, t13, a1);
//     T::polyeval_mla(x, t14, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval41<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
//     a27: T,
//     a28: T,
//     a29: T,
//     a30: T,
//     a31: T,
//     a32: T,
//     a33: T,
//     a34: T,
//     a35: T,
//     a36: T,
//     a37: T,
//     a38: T,
//     a39: T,
//     a40: T,
// ) -> T {
//     let mut acc = a40;
//     acc = T::polyeval_mla(x, acc, a39);
//     acc = T::polyeval_mla(x, acc, a38);
//     acc = T::polyeval_mla(x, acc, a37);
//     acc = T::polyeval_mla(x, acc, a36);
//     acc = T::polyeval_mla(x, acc, a35);
//     acc = T::polyeval_mla(x, acc, a34);
//     acc = T::polyeval_mla(x, acc, a33);
//     acc = T::polyeval_mla(x, acc, a32);
//     acc = T::polyeval_mla(x, acc, a31);
//     acc = T::polyeval_mla(x, acc, a30);
//     acc = T::polyeval_mla(x, acc, a29);
//     acc = T::polyeval_mla(x, acc, a28);
//     acc = T::polyeval_mla(x, acc, a27);
//     acc = T::polyeval_mla(x, acc, a26);
//     acc = T::polyeval_mla(x, acc, a25);
//     acc = T::polyeval_mla(x, acc, a24);
//     acc = T::polyeval_mla(x, acc, a23);
//     acc = T::polyeval_mla(x, acc, a22);
//     acc = T::polyeval_mla(x, acc, a21);
//     acc = T::polyeval_mla(x, acc, a20);
//     acc = T::polyeval_mla(x, acc, a19);
//     acc = T::polyeval_mla(x, acc, a18);
//     acc = T::polyeval_mla(x, acc, a17);
//     acc = T::polyeval_mla(x, acc, a16);
//     acc = T::polyeval_mla(x, acc, a15);
//     acc = T::polyeval_mla(x, acc, a14);
//     acc = T::polyeval_mla(x, acc, a13);
//     acc = T::polyeval_mla(x, acc, a12);
//     acc = T::polyeval_mla(x, acc, a11);
//     acc = T::polyeval_mla(x, acc, a10);
//     acc = T::polyeval_mla(x, acc, a9);
//     acc = T::polyeval_mla(x, acc, a8);
//     acc = T::polyeval_mla(x, acc, a7);
//     acc = T::polyeval_mla(x, acc, a6);
//     acc = T::polyeval_mla(x, acc, a5);
//     acc = T::polyeval_mla(x, acc, a4);
//     acc = T::polyeval_mla(x, acc, a3);
//     acc = T::polyeval_mla(x, acc, a2);
//     acc = T::polyeval_mla(x, acc, a1);
//     T::polyeval_mla(x, acc, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval44<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
//     a27: T,
//     a28: T,
//     a29: T,
//     a30: T,
//     a31: T,
//     a32: T,
//     a33: T,
//     a34: T,
//     a35: T,
//     a36: T,
//     a37: T,
//     a38: T,
//     a39: T,
//     a40: T,
//     a41: T,
//     a42: T,
//     a43: T,
// ) -> T {
//     let mut acc = a43;
//     acc = T::polyeval_mla(x, acc, a42);
//     acc = T::polyeval_mla(x, acc, a41);
//     acc = T::polyeval_mla(x, acc, a40);
//     acc = T::polyeval_mla(x, acc, a39);
//     acc = T::polyeval_mla(x, acc, a38);
//     acc = T::polyeval_mla(x, acc, a37);
//     acc = T::polyeval_mla(x, acc, a36);
//     acc = T::polyeval_mla(x, acc, a35);
//     acc = T::polyeval_mla(x, acc, a34);
//     acc = T::polyeval_mla(x, acc, a33);
//     acc = T::polyeval_mla(x, acc, a32);
//     acc = T::polyeval_mla(x, acc, a31);
//     acc = T::polyeval_mla(x, acc, a30);
//     acc = T::polyeval_mla(x, acc, a29);
//     acc = T::polyeval_mla(x, acc, a28);
//     acc = T::polyeval_mla(x, acc, a27);
//     acc = T::polyeval_mla(x, acc, a26);
//     acc = T::polyeval_mla(x, acc, a25);
//     acc = T::polyeval_mla(x, acc, a24);
//     acc = T::polyeval_mla(x, acc, a23);
//     acc = T::polyeval_mla(x, acc, a22);
//     acc = T::polyeval_mla(x, acc, a21);
//     acc = T::polyeval_mla(x, acc, a20);
//     acc = T::polyeval_mla(x, acc, a19);
//     acc = T::polyeval_mla(x, acc, a18);
//     acc = T::polyeval_mla(x, acc, a17);
//     acc = T::polyeval_mla(x, acc, a16);
//     acc = T::polyeval_mla(x, acc, a15);
//     acc = T::polyeval_mla(x, acc, a14);
//     acc = T::polyeval_mla(x, acc, a13);
//     acc = T::polyeval_mla(x, acc, a12);
//     acc = T::polyeval_mla(x, acc, a11);
//     acc = T::polyeval_mla(x, acc, a10);
//     acc = T::polyeval_mla(x, acc, a9);
//     acc = T::polyeval_mla(x, acc, a8);
//     acc = T::polyeval_mla(x, acc, a7);
//     acc = T::polyeval_mla(x, acc, a6);
//     acc = T::polyeval_mla(x, acc, a5);
//     acc = T::polyeval_mla(x, acc, a4);
//     acc = T::polyeval_mla(x, acc, a3);
//     acc = T::polyeval_mla(x, acc, a2);
//     acc = T::polyeval_mla(x, acc, a1);
//     T::polyeval_mla(x, acc, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval43<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
//     a27: T,
//     a28: T,
//     a29: T,
//     a30: T,
//     a31: T,
//     a32: T,
//     a33: T,
//     a34: T,
//     a35: T,
//     a36: T,
//     a37: T,
//     a38: T,
//     a39: T,
//     a40: T,
//     a41: T,
//     a42: T,
// ) -> T {
//     let z000 = T::polyeval_mla(x, a42, a41);
//     let z000 = T::polyeval_mla(x, z000, a40);
//     let z000 = T::polyeval_mla(x, z000, a39);
//     let z000 = T::polyeval_mla(x, z000, a38);
//     let z000 = T::polyeval_mla(x, z000, a37);
//     let z000 = T::polyeval_mla(x, z000, a36);
//     let z000 = T::polyeval_mla(x, z000, a35);
//     let z000 = T::polyeval_mla(x, z000, a34);
//     let z000 = T::polyeval_mla(x, z000, a33);
//     let z000 = T::polyeval_mla(x, z000, a32);
//     let z000 = T::polyeval_mla(x, z000, a31);
//     let z000 = T::polyeval_mla(x, z000, a30);
//     let z000 = T::polyeval_mla(x, z000, a29);
//     let z000 = T::polyeval_mla(x, z000, a28);
//     let z000 = T::polyeval_mla(x, z000, a27);
//     let z000 = T::polyeval_mla(x, z000, a26);
//     let z000 = T::polyeval_mla(x, z000, a25);
//     let z000 = T::polyeval_mla(x, z000, a24);
//     let z000 = T::polyeval_mla(x, z000, a23);
//     let z000 = T::polyeval_mla(x, z000, a22);
//     let z000 = T::polyeval_mla(x, z000, a21);
//     let z000 = T::polyeval_mla(x, z000, a20);
//     let z000 = T::polyeval_mla(x, z000, a19);
//     let z000 = T::polyeval_mla(x, z000, a18);
//     let z000 = T::polyeval_mla(x, z000, a17);
//     let z00 = T::polyeval_mla(x, z000, a16);
//     let z01 = T::polyeval_mla(x, z00, a15);
//     let t1 = T::polyeval_mla(x, z01, a14);
//     let t2 = T::polyeval_mla(x, t1, a13);
//     let t3 = T::polyeval_mla(x, t2, a12);
//     let t4 = T::polyeval_mla(x, t3, a11);
//     let t5 = T::polyeval_mla(x, t4, a10);
//     let t6 = T::polyeval_mla(x, t5, a9);
//     let t7 = T::polyeval_mla(x, t6, a8);
//     let t8 = T::polyeval_mla(x, t7, a7);
//     let t9 = T::polyeval_mla(x, t8, a6);
//     let t10 = T::polyeval_mla(x, t9, a5);
//     let t11 = T::polyeval_mla(x, t10, a4);
//     let t12 = T::polyeval_mla(x, t11, a3);
//     let t13 = T::polyeval_mla(x, t12, a2);
//     let t14 = T::polyeval_mla(x, t13, a1);
//     T::polyeval_mla(x, t14, a0)
// }

// #[inline(always)]
// #[allow(clippy::too_many_arguments)]
// pub(crate) fn f_polyeval35<T: PolyevalMla + Copy + Mul<T, Output = T>>(
//     x: T,
//     a0: T,
//     a1: T,
//     a2: T,
//     a3: T,
//     a4: T,
//     a5: T,
//     a6: T,
//     a7: T,
//     a8: T,
//     a9: T,
//     a10: T,
//     a11: T,
//     a12: T,
//     a13: T,
//     a14: T,
//     a15: T,
//     a16: T,
//     a17: T,
//     a18: T,
//     a19: T,
//     a20: T,
//     a21: T,
//     a22: T,
//     a23: T,
//     a24: T,
//     a25: T,
//     a26: T,
//     a27: T,
//     a28: T,
//     a29: T,
//     a30: T,
//     a31: T,
//     a32: T,
//     a33: T,
//     a34: T,
// ) -> T {
//     // let z000 = T::polyeval_mla(x, a34, a33);
//     // let z000 = T::polyeval_mla(x, z000, a32);
//     // let z000 = T::polyeval_mla(x, z000, a31);
//     // let z000 = T::polyeval_mla(x, z000, a30);
//     // let z000 = T::polyeval_mla(x, z000, a29);
//     // let z000 = T::polyeval_mla(x, z000, a28);
//     // let z000 = T::polyeval_mla(x, z000, a27);
//     // let z000 = T::polyeval_mla(x, z000, a26);
//     // let z000 = T::polyeval_mla(x, z000, a25);
//     // let z000 = T::polyeval_mla(x, z000, a24);
//     // let z000 = T::polyeval_mla(x, z000, a23);
//     // let z000 = T::polyeval_mla(x, z000, a22);
//     // let z000 = T::polyeval_mla(x, z000, a21);
//     // let z000 = T::polyeval_mla(x, z000, a20);
//     // let z000 = T::polyeval_mla(x, z000, a19);
//     // let z000 = T::polyeval_mla(x, z000, a18);
//     // let z000 = T::polyeval_mla(x, z000, a17);
//     // let z00 = T::polyeval_mla(x, z000, a16);
//     // let z01 = T::polyeval_mla(x, z00, a15);
//     // let t1 = T::polyeval_mla(x, z01, a14);
//     // let t2 = T::polyeval_mla(x, t1, a13);
//     // let t3 = T::polyeval_mla(x, t2, a12);
//     // let t4 = T::polyeval_mla(x, t3, a11);
//     // let t5 = T::polyeval_mla(x, t4, a10);
//     // let t6 = T::polyeval_mla(x, t5, a9);
//     // let t7 = T::polyeval_mla(x, t6, a8);
//     // let t8 = T::polyeval_mla(x, t7, a7);
//     // let t9 = T::polyeval_mla(x, t8, a6);
//     // let t10 = T::polyeval_mla(x, t9, a5);
//     // let t11 = T::polyeval_mla(x, t10, a4);
//     // let t12 = T::polyeval_mla(x, t11, a3);
//     // let t13 = T::polyeval_mla(x, t12, a2);
//     // let t14 = T::polyeval_mla(x, t13, a1);
//     // T::polyeval_mla(x, t14, a0)
//
//     let x2 = x * x;
//     let x4 = x2 * x2;
//     let x8 = x4 * x4;
//     let x16 = x8 * x8;
//     let x32 = x16 * x16;
//
//     // Level 0
//     let z0 = T::polyeval_mla(x, a1, a0);
//     let z1 = T::polyeval_mla(x, a3, a2);
//     let z2 = T::polyeval_mla(x, a5, a4);
//     let z3 = T::polyeval_mla(x, a7, a6);
//     let z4 = T::polyeval_mla(x, a9, a8);
//     let z5 = T::polyeval_mla(x, a11, a10);
//     let z6 = T::polyeval_mla(x, a13, a12);
//     let z7 = T::polyeval_mla(x, a15, a14);
//     let z8 = T::polyeval_mla(x, a17, a16);
//     let z9 = T::polyeval_mla(x, a19, a18);
//     let z10 = T::polyeval_mla(x, a21, a20);
//     let z11 = T::polyeval_mla(x, a23, a22);
//     let z12 = T::polyeval_mla(x, a25, a24);
//     let z13 = T::polyeval_mla(x, a27, a26);
//     let z14 = T::polyeval_mla(x, a29, a28);
//     let z15 = T::polyeval_mla(x, a31, a30);
//     let z16 = T::polyeval_mla(x, a33, a32);
//     let z17 = a34;
//
//     // Level 1
//     let y0 = T::polyeval_mla(x2, z1, z0);
//     let y1 = T::polyeval_mla(x2, z3, z2);
//     let y2 = T::polyeval_mla(x2, z5, z4);
//     let y3 = T::polyeval_mla(x2, z7, z6);
//     let y4 = T::polyeval_mla(x2, z9, z8);
//     let y5 = T::polyeval_mla(x2, z11, z10);
//     let y6 = T::polyeval_mla(x2, z13, z12);
//     let y7 = T::polyeval_mla(x2, z15, z14);
//     let y8 = T::polyeval_mla(x2, z17, z16);
//
//     // Level 2
//     let w0 = T::polyeval_mla(x4, y1, y0);
//     let w1 = T::polyeval_mla(x4, y3, y2);
//     let w2 = T::polyeval_mla(x4, y5, y4);
//     let w3 = T::polyeval_mla(x4, y7, y6);
//     let w4 = y8;
//
//     // Level 3
//     let v0 = T::polyeval_mla(x8, w1, w0);
//     let v1 = T::polyeval_mla(x8, w3, w2);
//     let v2 = w4;
//
//     // Level 4
//     let u0 = T::polyeval_mla(x16, v1, v0);
//     let u1 = v2;
//
//     // Level 5 (final)
//     T::polyeval_mla(x32, u1, u0)
// }
