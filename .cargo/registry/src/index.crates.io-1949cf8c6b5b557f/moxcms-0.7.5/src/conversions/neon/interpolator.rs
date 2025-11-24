/*
 * // Copyright (c) Radzivon Bartoshyk 3/2025. All rights reserved.
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
#![allow(dead_code)]
use crate::conversions::interpolator::BarycentricWeight;
use crate::conversions::neon::rgb_xyz::NeonAlignedF32;
use crate::math::{FusedMultiplyAdd, FusedMultiplyNegAdd};
use num_traits::AsPrimitive;
use std::arch::aarch64::*;
use std::ops::{Add, Mul, Sub};

pub(crate) struct TetrahedralNeon<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [NeonAlignedF32],
}

pub(crate) struct PyramidalNeon<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [NeonAlignedF32],
}

pub(crate) struct TrilinearNeon<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [NeonAlignedF32],
}

pub(crate) struct PyramidalNeonDouble<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [NeonAlignedF32],
    pub(crate) cube1: &'a [NeonAlignedF32],
}

pub(crate) struct PrismaticNeonDouble<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [NeonAlignedF32],
    pub(crate) cube1: &'a [NeonAlignedF32],
}

pub(crate) struct TrilinearNeonDouble<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [NeonAlignedF32],
    pub(crate) cube1: &'a [NeonAlignedF32],
}

pub(crate) struct TetrahedralNeonDouble<'a, const GRID_SIZE: usize> {
    pub(crate) cube0: &'a [NeonAlignedF32],
    pub(crate) cube1: &'a [NeonAlignedF32],
}

pub(crate) struct PrismaticNeon<'a, const GRID_SIZE: usize> {
    pub(crate) cube: &'a [NeonAlignedF32],
}

trait Fetcher<T> {
    fn fetch(&self, x: i32, y: i32, z: i32) -> T;
}

struct TetrahedralNeonFetchVector<'a, const GRID_SIZE: usize> {
    cube: &'a [NeonAlignedF32],
}

struct TetrahedralNeonFetchVectorDouble<'a, const GRID_SIZE: usize> {
    cube0: &'a [NeonAlignedF32],
    cube1: &'a [NeonAlignedF32],
}

#[derive(Copy, Clone)]
pub(crate) struct NeonVector {
    pub(crate) v: float32x4_t,
}

#[derive(Copy, Clone)]
pub(crate) struct NeonVectorDouble {
    pub(crate) v0: float32x4_t,
    pub(crate) v1: float32x4_t,
}

impl From<f32> for NeonVector {
    #[inline(always)]
    fn from(v: f32) -> Self {
        NeonVector {
            v: unsafe { vdupq_n_f32(v) },
        }
    }
}

impl From<f32> for NeonVectorDouble {
    #[inline(always)]
    fn from(v: f32) -> Self {
        NeonVectorDouble {
            v0: unsafe { vdupq_n_f32(v) },
            v1: unsafe { vdupq_n_f32(v) },
        }
    }
}

impl Sub<NeonVector> for NeonVector {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: NeonVector) -> Self::Output {
        NeonVector {
            v: unsafe { vsubq_f32(self.v, rhs.v) },
        }
    }
}

impl Mul<NeonVector> for NeonVector {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: NeonVector) -> Self::Output {
        NeonVector {
            v: unsafe { vmulq_f32(self.v, rhs.v) },
        }
    }
}

impl Sub<NeonVectorDouble> for NeonVectorDouble {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: NeonVectorDouble) -> Self::Output {
        NeonVectorDouble {
            v0: unsafe { vsubq_f32(self.v0, rhs.v0) },
            v1: unsafe { vsubq_f32(self.v1, rhs.v1) },
        }
    }
}

impl Mul<NeonVectorDouble> for NeonVectorDouble {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: NeonVectorDouble) -> Self::Output {
        NeonVectorDouble {
            v0: unsafe { vmulq_f32(self.v0, rhs.v0) },
            v1: unsafe { vmulq_f32(self.v1, rhs.v1) },
        }
    }
}

impl Add<NeonVector> for NeonVector {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: NeonVector) -> Self::Output {
        NeonVector {
            v: unsafe { vaddq_f32(self.v, rhs.v) },
        }
    }
}

impl Add<NeonVectorDouble> for NeonVectorDouble {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: NeonVectorDouble) -> Self::Output {
        NeonVectorDouble {
            v0: unsafe { vaddq_f32(self.v0, rhs.v0) },
            v1: unsafe { vaddq_f32(self.v1, rhs.v1) },
        }
    }
}

impl FusedMultiplyAdd<NeonVector> for NeonVector {
    #[inline(always)]
    fn mla(&self, b: NeonVector, c: NeonVector) -> NeonVector {
        NeonVector {
            v: unsafe { vfmaq_f32(self.v, b.v, c.v) },
        }
    }
}

impl FusedMultiplyNegAdd<NeonVector> for NeonVector {
    #[inline(always)]
    fn neg_mla(&self, b: NeonVector, c: NeonVector) -> NeonVector {
        NeonVector {
            v: unsafe { vfmsq_f32(self.v, b.v, c.v) },
        }
    }
}

impl NeonVectorDouble {
    #[inline(always)]
    fn neg_mla(&self, b: NeonVectorDouble, c: NeonVectorDouble) -> NeonVectorDouble {
        NeonVectorDouble {
            v0: unsafe { vfmsq_f32(self.v0, b.v0, c.v0) },
            v1: unsafe { vfmsq_f32(self.v1, b.v1, c.v1) },
        }
    }
}

impl NeonVectorDouble {
    #[inline(always)]
    fn mla(&self, b: NeonVectorDouble, c: NeonVector) -> NeonVectorDouble {
        NeonVectorDouble {
            v0: unsafe { vfmaq_f32(self.v0, b.v0, c.v) },
            v1: unsafe { vfmaq_f32(self.v1, b.v1, c.v) },
        }
    }

    #[inline(always)]
    pub(crate) fn split(self) -> (NeonVector, NeonVector) {
        (NeonVector { v: self.v0 }, NeonVector { v: self.v1 })
    }
}

impl<const GRID_SIZE: usize> Fetcher<NeonVector> for TetrahedralNeonFetchVector<'_, GRID_SIZE> {
    fn fetch(&self, x: i32, y: i32, z: i32) -> NeonVector {
        let offset = (x as u32 * (GRID_SIZE as u32 * GRID_SIZE as u32)
            + y as u32 * GRID_SIZE as u32
            + z as u32) as usize;
        let jx = unsafe { self.cube.get_unchecked(offset..) };
        NeonVector {
            v: unsafe { vld1q_f32(jx.as_ptr() as *const f32) },
        }
    }
}

impl<const GRID_SIZE: usize> Fetcher<NeonVectorDouble>
    for TetrahedralNeonFetchVectorDouble<'_, GRID_SIZE>
{
    fn fetch(&self, x: i32, y: i32, z: i32) -> NeonVectorDouble {
        let offset = (x as u32 * (GRID_SIZE as u32 * GRID_SIZE as u32)
            + y as u32 * GRID_SIZE as u32
            + z as u32) as usize;
        let jx0 = unsafe { self.cube0.get_unchecked(offset..) };
        let jx1 = unsafe { self.cube1.get_unchecked(offset..) };
        NeonVectorDouble {
            v0: unsafe { vld1q_f32(jx0.as_ptr() as *const f32) },
            v1: unsafe { vld1q_f32(jx1.as_ptr() as *const f32) },
        }
    }
}

pub(crate) trait NeonMdInterpolation<'a, const GRID_SIZE: usize> {
    fn new(table: &'a [NeonAlignedF32]) -> Self;
    fn inter3_neon<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
    ) -> NeonVector;
}

pub(crate) trait NeonMdInterpolationDouble<'a, const GRID_SIZE: usize> {
    fn new(table0: &'a [NeonAlignedF32], table1: &'a [NeonAlignedF32]) -> Self;
    fn inter3_neon<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
    ) -> (NeonVector, NeonVector);
}

impl<const GRID_SIZE: usize> TetrahedralNeon<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r: impl Fetcher<NeonVector>,
    ) -> NeonVector {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let rx = lut_r.w;
        let ry = lut_g.w;
        let rz = lut_b.w;

        let c0 = r.fetch(x, y, z);

        let c2;
        let c1;
        let c3;
        if rx >= ry {
            if ry >= rz {
                //rx >= ry && ry >= rz
                c1 = r.fetch(x_n, y, z) - c0;
                c2 = r.fetch(x_n, y_n, z) - r.fetch(x_n, y, z);
                c3 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y_n, z);
            } else if rx >= rz {
                //rx >= rz && rz >= ry
                c1 = r.fetch(x_n, y, z) - c0;
                c2 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y, z_n);
                c3 = r.fetch(x_n, y, z_n) - r.fetch(x_n, y, z);
            } else {
                //rz > rx && rx >= ry
                c1 = r.fetch(x_n, y, z_n) - r.fetch(x, y, z_n);
                c2 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y, z_n);
                c3 = r.fetch(x, y, z_n) - c0;
            }
        } else if rx >= rz {
            //ry > rx && rx >= rz
            c1 = r.fetch(x_n, y_n, z) - r.fetch(x, y_n, z);
            c2 = r.fetch(x, y_n, z) - c0;
            c3 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y_n, z);
        } else if ry >= rz {
            //ry >= rz && rz > rx
            c1 = r.fetch(x_n, y_n, z_n) - r.fetch(x, y_n, z_n);
            c2 = r.fetch(x, y_n, z) - c0;
            c3 = r.fetch(x, y_n, z_n) - r.fetch(x, y_n, z);
        } else {
            //rz > ry && ry > rx
            c1 = r.fetch(x_n, y_n, z_n) - r.fetch(x, y_n, z_n);
            c2 = r.fetch(x, y_n, z_n) - r.fetch(x, y, z_n);
            c3 = r.fetch(x, y, z_n) - c0;
        }
        let s0 = c0.mla(c1, NeonVector::from(rx));
        let s1 = s0.mla(c2, NeonVector::from(ry));
        s1.mla(c3, NeonVector::from(rz))
    }
}

impl<const GRID_SIZE: usize> TetrahedralNeonDouble<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r: impl Fetcher<NeonVectorDouble>,
    ) -> (NeonVector, NeonVector) {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let rx = lut_r.w;
        let ry = lut_g.w;
        let rz = lut_b.w;

        let c0 = r.fetch(x, y, z);

        let c2;
        let c1;
        let c3;
        if rx >= ry {
            if ry >= rz {
                //rx >= ry && ry >= rz
                c1 = r.fetch(x_n, y, z) - c0;
                c2 = r.fetch(x_n, y_n, z) - r.fetch(x_n, y, z);
                c3 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y_n, z);
            } else if rx >= rz {
                //rx >= rz && rz >= ry
                c1 = r.fetch(x_n, y, z) - c0;
                c2 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y, z_n);
                c3 = r.fetch(x_n, y, z_n) - r.fetch(x_n, y, z);
            } else {
                //rz > rx && rx >= ry
                c1 = r.fetch(x_n, y, z_n) - r.fetch(x, y, z_n);
                c2 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y, z_n);
                c3 = r.fetch(x, y, z_n) - c0;
            }
        } else if rx >= rz {
            //ry > rx && rx >= rz
            c1 = r.fetch(x_n, y_n, z) - r.fetch(x, y_n, z);
            c2 = r.fetch(x, y_n, z) - c0;
            c3 = r.fetch(x_n, y_n, z_n) - r.fetch(x_n, y_n, z);
        } else if ry >= rz {
            //ry >= rz && rz > rx
            c1 = r.fetch(x_n, y_n, z_n) - r.fetch(x, y_n, z_n);
            c2 = r.fetch(x, y_n, z) - c0;
            c3 = r.fetch(x, y_n, z_n) - r.fetch(x, y_n, z);
        } else {
            //rz > ry && ry > rx
            c1 = r.fetch(x_n, y_n, z_n) - r.fetch(x, y_n, z_n);
            c2 = r.fetch(x, y_n, z_n) - r.fetch(x, y, z_n);
            c3 = r.fetch(x, y, z_n) - c0;
        }
        let s0 = c0.mla(c1, NeonVector::from(rx));
        let s1 = s0.mla(c2, NeonVector::from(ry));
        s1.mla(c3, NeonVector::from(rz)).split()
    }
}

macro_rules! define_md_inter_neon {
    ($interpolator: ident) => {
        impl<'a, const GRID_SIZE: usize> NeonMdInterpolation<'a, GRID_SIZE>
            for $interpolator<'a, GRID_SIZE>
        {
            #[inline(always)]
            fn new(table: &'a [NeonAlignedF32]) -> Self {
                Self { cube: table }
            }

            #[inline(always)]
            fn inter3_neon<U: AsPrimitive<usize>, const BINS: usize>(
                &self,
                in_r: U,
                in_g: U,
                in_b: U,
                lut: &[BarycentricWeight<f32>; BINS],
            ) -> NeonVector {
                self.interpolate(
                    in_r,
                    in_g,
                    in_b,
                    lut,
                    TetrahedralNeonFetchVector::<GRID_SIZE> { cube: self.cube },
                )
            }
        }
    };
}

macro_rules! define_md_inter_neon_d {
    ($interpolator: ident) => {
        impl<'a, const GRID_SIZE: usize> NeonMdInterpolationDouble<'a, GRID_SIZE>
            for $interpolator<'a, GRID_SIZE>
        {
            #[inline(always)]
            fn new(table0: &'a [NeonAlignedF32], table1: &'a [NeonAlignedF32]) -> Self {
                Self {
                    cube0: table0,
                    cube1: table1,
                }
            }

            #[inline(always)]
            fn inter3_neon<U: AsPrimitive<usize>, const BINS: usize>(
                &self,
                in_r: U,
                in_g: U,
                in_b: U,
                lut: &[BarycentricWeight<f32>; BINS],
            ) -> (NeonVector, NeonVector) {
                self.interpolate(
                    in_r,
                    in_g,
                    in_b,
                    lut,
                    TetrahedralNeonFetchVectorDouble::<GRID_SIZE> {
                        cube0: self.cube0,
                        cube1: self.cube1,
                    },
                )
            }
        }
    };
}

define_md_inter_neon!(TetrahedralNeon);
define_md_inter_neon!(PyramidalNeon);
define_md_inter_neon!(PrismaticNeon);
define_md_inter_neon!(TrilinearNeon);
define_md_inter_neon_d!(PrismaticNeonDouble);
define_md_inter_neon_d!(PyramidalNeonDouble);
define_md_inter_neon_d!(TetrahedralNeonDouble);
define_md_inter_neon_d!(TrilinearNeonDouble);

impl<const GRID_SIZE: usize> PyramidalNeon<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r: impl Fetcher<NeonVector>,
    ) -> NeonVector {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let dr = lut_r.w;
        let dg = lut_g.w;
        let db = lut_b.w;

        let c0 = r.fetch(x, y, z);

        if dr > db && dg > db {
            let x0 = r.fetch(x_n, y_n, z_n);
            let x1 = r.fetch(x_n, y_n, z);
            let x2 = r.fetch(x_n, y, z);
            let x3 = r.fetch(x, y_n, z);

            let c1 = x0 - x1;
            let c2 = x2 - c0;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x2 + x1;

            let s0 = c0.mla(c1, NeonVector::from(db));
            let s1 = s0.mla(c2, NeonVector::from(dr));
            let s2 = s1.mla(c3, NeonVector::from(dg));
            s2.mla(c4, NeonVector::from(dr * dg))
        } else if db > dr && dg > dr {
            let x0 = r.fetch(x, y, z_n);
            let x1 = r.fetch(x_n, y_n, z_n);
            let x2 = r.fetch(x, y_n, z_n);
            let x3 = r.fetch(x, y_n, z);

            let c1 = x0 - c0;
            let c2 = x1 - x2;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x0 + x2;

            let s0 = c0.mla(c1, NeonVector::from(db));
            let s1 = s0.mla(c2, NeonVector::from(dr));
            let s2 = s1.mla(c3, NeonVector::from(dg));
            s2.mla(c4, NeonVector::from(dg * db))
        } else {
            let x0 = r.fetch(x, y, z_n);
            let x1 = r.fetch(x_n, y, z);
            let x2 = r.fetch(x_n, y, z_n);
            let x3 = r.fetch(x_n, y_n, z_n);

            let c1 = x0 - c0;
            let c2 = x1 - c0;
            let c3 = x3 - x2;
            let c4 = c0 - x1 - x0 + x2;

            let s0 = c0.mla(c1, NeonVector::from(db));
            let s1 = s0.mla(c2, NeonVector::from(dr));
            let s2 = s1.mla(c3, NeonVector::from(dg));
            s2.mla(c4, NeonVector::from(db * dr))
        }
    }
}

impl<const GRID_SIZE: usize> PyramidalNeonDouble<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r: impl Fetcher<NeonVectorDouble>,
    ) -> (NeonVector, NeonVector) {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let dr = lut_r.w;
        let dg = lut_g.w;
        let db = lut_b.w;

        let c0 = r.fetch(x, y, z);

        let w0 = NeonVector::from(db);
        let w1 = NeonVector::from(dr);
        let w2 = NeonVector::from(dg);

        if dr > db && dg > db {
            let x0 = r.fetch(x_n, y_n, z_n);
            let x1 = r.fetch(x_n, y_n, z);
            let x2 = r.fetch(x_n, y, z);
            let x3 = r.fetch(x, y_n, z);

            let c1 = x0 - x1;
            let c2 = x2 - c0;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x2 + x1;

            let w3 = NeonVector::from(dr * dg);

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3).split()
        } else if db > dr && dg > dr {
            let x0 = r.fetch(x, y, z_n);
            let x1 = r.fetch(x_n, y_n, z_n);
            let x2 = r.fetch(x, y_n, z_n);
            let x3 = r.fetch(x, y_n, z);

            let c1 = x0 - c0;
            let c2 = x1 - x2;
            let c3 = x3 - c0;
            let c4 = c0 - x3 - x0 + x2;

            let w3 = NeonVector::from(dg * db);

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3).split()
        } else {
            let x0 = r.fetch(x, y, z_n);
            let x1 = r.fetch(x_n, y, z);
            let x2 = r.fetch(x_n, y, z_n);
            let x3 = r.fetch(x_n, y_n, z_n);

            let c1 = x0 - c0;
            let c2 = x1 - c0;
            let c3 = x3 - x2;
            let c4 = c0 - x1 - x0 + x2;

            let w3 = NeonVector::from(db * dr);

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            s2.mla(c4, w3).split()
        }
    }
}

impl<const GRID_SIZE: usize> PrismaticNeon<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r: impl Fetcher<NeonVector>,
    ) -> NeonVector {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let dr = lut_r.w;
        let dg = lut_g.w;
        let db = lut_b.w;

        let c0 = r.fetch(x, y, z);

        if db > dr {
            let x0 = r.fetch(x, y, z_n);
            let x1 = r.fetch(x_n, y, z_n);
            let x2 = r.fetch(x, y_n, z);
            let x3 = r.fetch(x, y_n, z_n);
            let x4 = r.fetch(x_n, y_n, z_n);

            let c1 = x0 - c0;
            let c2 = x1 - x0;
            let c3 = x2 - c0;
            let c4 = c0 - x2 - x0 + x3;
            let c5 = x0 - x3 - x1 + x4;

            let s0 = c0.mla(c1, NeonVector::from(db));
            let s1 = s0.mla(c2, NeonVector::from(dr));
            let s2 = s1.mla(c3, NeonVector::from(dg));
            let s3 = s2.mla(c4, NeonVector::from(dg * db));
            s3.mla(c5, NeonVector::from(dr * dg))
        } else {
            let x0 = r.fetch(x_n, y, z);
            let x1 = r.fetch(x_n, y, z_n);
            let x2 = r.fetch(x, y_n, z);
            let x3 = r.fetch(x_n, y_n, z);
            let x4 = r.fetch(x_n, y_n, z_n);

            let c1 = x1 - x0;
            let c2 = x0 - c0;
            let c3 = x2 - c0;
            let c4 = x0 - x3 - x1 + x4;
            let c5 = c0 - x2 - x0 + x3;

            let s0 = c0.mla(c1, NeonVector::from(db));
            let s1 = s0.mla(c2, NeonVector::from(dr));
            let s2 = s1.mla(c3, NeonVector::from(dg));
            let s3 = s2.mla(c4, NeonVector::from(dg * db));
            s3.mla(c5, NeonVector::from(dr * dg))
        }
    }
}

impl<const GRID_SIZE: usize> PrismaticNeonDouble<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        rv: impl Fetcher<NeonVectorDouble>,
    ) -> (NeonVector, NeonVector) {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let dr = lut_r.w;
        let dg = lut_g.w;
        let db = lut_b.w;

        let c0 = rv.fetch(x, y, z);

        let w0 = NeonVector::from(db);
        let w1 = NeonVector::from(dr);
        let w2 = NeonVector::from(dg);
        let w3 = NeonVector::from(dg * db);
        let w4 = NeonVector::from(dr * dg);

        if db > dr {
            let x0 = rv.fetch(x, y, z_n);
            let x1 = rv.fetch(x_n, y, z_n);
            let x2 = rv.fetch(x, y_n, z);
            let x3 = rv.fetch(x, y_n, z_n);
            let x4 = rv.fetch(x_n, y_n, z_n);

            let c1 = x0 - c0;
            let c2 = x1 - x0;
            let c3 = x2 - c0;
            let c4 = c0 - x2 - x0 + x3;
            let c5 = x0 - x3 - x1 + x4;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            let s3 = s2.mla(c4, w3);
            s3.mla(c5, w4).split()
        } else {
            let x0 = rv.fetch(x_n, y, z);
            let x1 = rv.fetch(x_n, y, z_n);
            let x2 = rv.fetch(x, y_n, z);
            let x3 = rv.fetch(x_n, y_n, z);
            let x4 = rv.fetch(x_n, y_n, z_n);

            let c1 = x1 - x0;
            let c2 = x0 - c0;
            let c3 = x2 - c0;
            let c4 = x0 - x3 - x1 + x4;
            let c5 = c0 - x2 - x0 + x3;

            let s0 = c0.mla(c1, w0);
            let s1 = s0.mla(c2, w1);
            let s2 = s1.mla(c3, w2);
            let s3 = s2.mla(c4, w3);
            s3.mla(c5, w4).split()
        }
    }
}

impl<const GRID_SIZE: usize> TrilinearNeonDouble<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r: impl Fetcher<NeonVectorDouble>,
    ) -> (NeonVector, NeonVector) {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let dr = lut_r.w;
        let dg = lut_g.w;
        let db = lut_b.w;

        let w0 = NeonVector::from(dr);
        let w1 = NeonVector::from(dg);
        let w2 = NeonVector::from(db);

        let c000 = r.fetch(x, y, z);
        let c100 = r.fetch(x_n, y, z);
        let c010 = r.fetch(x, y_n, z);
        let c110 = r.fetch(x_n, y_n, z);
        let c001 = r.fetch(x, y, z_n);
        let c101 = r.fetch(x_n, y, z_n);
        let c011 = r.fetch(x, y_n, z_n);
        let c111 = r.fetch(x_n, y_n, z_n);

        let dx = NeonVectorDouble::from(dr);

        let c00 = c000.neg_mla(c000, dx).mla(c100, w0);
        let c10 = c010.neg_mla(c010, dx).mla(c110, w0);
        let c01 = c001.neg_mla(c001, dx).mla(c101, w0);
        let c11 = c011.neg_mla(c011, dx).mla(c111, w0);

        let dy = NeonVectorDouble::from(dg);

        let c0 = c00.neg_mla(c00, dy).mla(c10, w1);
        let c1 = c01.neg_mla(c01, dy).mla(c11, w1);

        let dz = NeonVectorDouble::from(db);

        c0.neg_mla(c0, dz).mla(c1, w2).split()
    }
}

impl<const GRID_SIZE: usize> TrilinearNeon<'_, GRID_SIZE> {
    #[inline(always)]
    fn interpolate<U: AsPrimitive<usize>, const BINS: usize>(
        &self,
        in_r: U,
        in_g: U,
        in_b: U,
        lut: &[BarycentricWeight<f32>; BINS],
        r: impl Fetcher<NeonVector>,
    ) -> NeonVector {
        let lut_r = lut[in_r.as_()];
        let lut_g = lut[in_g.as_()];
        let lut_b = lut[in_b.as_()];

        let x: i32 = lut_r.x;
        let y: i32 = lut_g.x;
        let z: i32 = lut_b.x;

        let x_n: i32 = lut_r.x_n;
        let y_n: i32 = lut_g.x_n;
        let z_n: i32 = lut_b.x_n;

        let dr = lut_r.w;
        let dg = lut_g.w;
        let db = lut_b.w;

        let w0 = NeonVector::from(dr);
        let w1 = NeonVector::from(dg);
        let w2 = NeonVector::from(db);

        let c000 = r.fetch(x, y, z);
        let c100 = r.fetch(x_n, y, z);
        let c010 = r.fetch(x, y_n, z);
        let c110 = r.fetch(x_n, y_n, z);
        let c001 = r.fetch(x, y, z_n);
        let c101 = r.fetch(x_n, y, z_n);
        let c011 = r.fetch(x, y_n, z_n);
        let c111 = r.fetch(x_n, y_n, z_n);

        let dx = NeonVector::from(dr);

        let c00 = c000.neg_mla(c000, dx).mla(c100, w0);
        let c10 = c010.neg_mla(c010, dx).mla(c110, w0);
        let c01 = c001.neg_mla(c001, dx).mla(c101, w0);
        let c11 = c011.neg_mla(c011, dx).mla(c111, w0);

        let dy = NeonVector::from(dg);

        let c0 = c00.neg_mla(c00, dy).mla(c10, w1);
        let c1 = c01.neg_mla(c01, dy).mla(c11, w1);

        let dz = NeonVector::from(db);

        c0.neg_mla(c0, dz).mla(c1, w2)
    }
}
